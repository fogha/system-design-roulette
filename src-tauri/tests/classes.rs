use rusqlite::{params, Connection};
use system_design_roulette_lib::{
    catalog, classroom, db,
    domain::{
        classes::{self, AcceptPath},
        enrollment::{self, EntryChoice, LearningGoal, SaveEnrollmentDraft},
        placement,
    },
    language,
};

fn fixture() -> (std::path::PathBuf, Connection) {
    let path = std::env::temp_dir().join(format!(
        "principia-class-{:032x}.db",
        rand::random::<u128>()
    ));
    let conn = db::open(&path).unwrap();
    db::seed_concepts(&conn, include_str!("../seed/concepts.json")).unwrap();
    language::initialize(&conn, "2026-09-09").unwrap();
    classroom::initialize(&conn).unwrap();
    (path, conn)
}
fn setup(conn: &Connection, course: &str, entry: &str) -> AcceptPath {
    let options = enrollment::options(course).unwrap();
    let mut configuration = options.default_configuration;
    configuration.entry = EntryChoice::Manual {
        entry_point: entry.into(),
        familiar_competencies: vec![],
    };
    if let LearningGoal::LanguageLevel { target_level, .. } = &mut configuration.goal {
        *target_level = "B2".into();
    }
    let draft = enrollment::save_draft(
        conn,
        &SaveEnrollmentDraft {
            id: None,
            expected_revision: None,
            course: options.course,
            configuration,
        },
    )
    .unwrap();
    let recommendation = placement::recommend(conn, &draft.id, draft.revision).unwrap();
    AcceptPath {
        draft_id: draft.id,
        expected_revision: draft.revision,
        recommendation_id: recommendation.id,
    }
}
fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

#[test]
fn all_nine_paths_save_without_activating_unscheduled_classes_or_awarding_credit() {
    let (file, conn) = fixture();
    let tables = [
        "sessions",
        "classroom_sessions",
        "language_sessions",
        "mastery",
        "attempts",
        "assessment_attempts",
        "classroom_schedule_slots",
    ];
    let before: Vec<_> = tables.iter().map(|t| count(&conn, t)).collect();
    let mut accepted = vec![];
    for course in catalog::COURSES {
        let request = setup(&conn, course.course_id, course.entry_points[1].id);
        let path = classes::accept(&conn, &request, "2026-09-09").unwrap();
        let repeated = classes::accept(&conn, &request, "2026-09-10").unwrap();
        assert_eq!(
            serde_json::to_value(&path).unwrap(),
            serde_json::to_value(repeated).unwrap()
        );
        assert_eq!(path.recommendation.required_outcome, course.outcome);
        if course.kind == catalog::SubjectKind::Language {
            assert!(language::program_view(&conn, course.id, "2026-09-09")
                .unwrap()
                .milestones
                .iter()
                .all(|m| !m.reached));
        }
        assert!(!classroom::program_row(&conn, course.id).unwrap().enabled);
        assert_eq!(
            classroom::program_view(&conn, course.id, "2026-09-09")
                .unwrap()
                .accepted_path
                .unwrap()
                .entry_point,
            course.entry_points[1].id
        );
        accepted.push(path);
    }
    assert_eq!(count(&conn, "classes"), 9);
    assert_eq!(count(&conn, "path_revisions"), 9);
    for (table, before) in tables.iter().zip(before) {
        assert_eq!(count(&conn, table), before, "acceptance changed {table}");
    }
    assert_eq!(
        conn.query_row("PRAGMA foreign_key_check", [], |_| Ok(()))
            .optional()
            .unwrap(),
        None
    );
    drop(conn);
    let conn = db::open(&file).unwrap();
    for expected in accepted {
        assert_eq!(
            classes::current_path(&conn, &expected.recommendation.course.course_id)
                .unwrap()
                .unwrap()
                .reference,
            expected.reference
        );
    }
}

use rusqlite::OptionalExtension;
#[test]
fn stale_recommendation_and_failed_adapter_write_leave_no_partial_enrollment() {
    let (_, conn) = fixture();
    let mut input = setup(&conn, "linux-bash", "mechanisms");
    let original = input.recommendation_id.clone();
    input.recommendation_id = "stale".into();
    assert!(classes::accept(&conn, &input, "2026-09-09").is_err());
    assert_eq!(count(&conn, "classes"), 0);
    assert!(!classroom::program_row(&conn, "linux-bash").unwrap().enabled);
    input.recommendation_id = original;
    conn.execute_batch("CREATE TRIGGER fail_accept BEFORE INSERT ON path_revisions BEGIN SELECT RAISE(ABORT,'injected write failure'); END;").unwrap();
    assert!(classes::accept(&conn, &input, "2026-09-09").is_err());
    assert_eq!(count(&conn, "classes"), 0);
    assert!(!classroom::program_row(&conn, "linux-bash").unwrap().enabled);
    assert_eq!(
        enrollment::draft(&conn, &input.draft_id).unwrap().status,
        enrollment::DraftStatus::Draft
    );
}

#[test]
fn engineering_selection_starts_in_accepted_phase_without_awarding_prerequisite_mastery() {
    let (_, conn) = fixture();
    for course in catalog::COURSES
        .iter()
        .filter(|c| c.kind == catalog::SubjectKind::Engineering)
    {
        let input = setup(&conn, course.course_id, "mechanisms");
        let accepted = classes::accept(&conn, &input, "2026-09-09").unwrap();
        let concept = classes::next_concept(&conn, course.course_id, "2026-09-09")
            .unwrap()
            .unwrap();
        assert_eq!(
            concept.curriculum.phase, "mechanisms",
            "{} returned {}",
            course.id, concept.slug
        );
        assert!(!accepted
            .recommendation
            .earlier_topics
            .iter()
            .any(|t| t.id == concept.slug));
        assert!(!accepted.recommendation.refreshers.is_empty());
    }
    assert_eq!(count(&conn, "mastery"), 0);
    assert_eq!(count(&conn, "classroom_sessions"), 0);
}

#[test]
fn reassessment_keeps_the_class_and_prior_path_and_the_original_retry_result() {
    let (_, conn) = fixture();
    let first_input = setup(&conn, "bash-scripting", "foundations");
    let first = classes::accept(&conn, &first_input, "2026-09-09").unwrap();
    let second_input = setup(&conn, "bash-scripting", "production");
    let second = classes::accept(&conn, &second_input, "2026-09-10").unwrap();
    assert_eq!(first.reference.class_id, second.reference.class_id);
    assert_ne!(
        first.reference.path_revision_id,
        second.reference.path_revision_id
    );
    assert_eq!(second.revision, 2);
    assert_eq!(
        classes::accept(&conn, &first_input, "2026-09-11")
            .unwrap()
            .reference,
        first.reference
    );
    assert_eq!(
        classes::current_path(&conn, "bash-scripting")
            .unwrap()
            .unwrap()
            .reference,
        second.reference
    );
    assert_eq!(count(&conn, "classes"), 1);
    assert_eq!(count(&conn, "path_revisions"), 2);
    assert_eq!(
        classes::next_concept(&conn, "bash-scripting", "2026-09-11")
            .unwrap()
            .unwrap()
            .curriculum
            .phase,
        "production"
    );
}

#[test]
fn a_new_language_path_changes_future_entry_without_rewriting_an_active_lesson() {
    let (_, conn) = fixture();
    classroom::upsert_slot(
        &conn,
        &classroom::UpsertClassroomSlotInput {
            id: None,
            subject_id: "german".into(),
            hour: 7,
            minute: 30,
            weekdays: vec![1, 3, 5],
            enabled: true,
        },
    )
    .unwrap();
    let first_input = setup(&conn, "german", "A1");
    let first = classes::accept(&conn, &first_input, "2026-09-09").unwrap();
    let lesson = language::start_session(&conn, "german", "2026-09-09", false).unwrap();
    let before: String = conn
        .query_row(
            "SELECT lesson_json FROM language_sessions WHERE id=?1",
            [lesson.session_id.parse::<i64>().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&before).unwrap()["path"]["path_revision_id"],
        first.reference.path_revision_id
    );
    let second_input = setup(&conn, "german", "B1");
    classes::accept(&conn, &second_input, "2026-09-10").unwrap();
    let resumed = language::start_session(&conn, "german", "2026-09-10", false).unwrap();
    assert_eq!(resumed.session_id, lesson.session_id);
    assert_eq!(resumed.level, "A1");
    let after: String = conn
        .query_row(
            "SELECT lesson_json FROM language_sessions WHERE id=?1",
            [lesson.session_id.parse::<i64>().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(before, after);
    let (baseline, cursor): (String, String) = conn
        .query_row(
            "SELECT start_level,current_level FROM language_programs WHERE language='german'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(baseline, "A1");
    assert_eq!(cursor, "B1");
    // A fixture terminal state leaves the next start free to use the accepted cursor.
    conn.execute(
        "UPDATE language_sessions SET status='completed' WHERE id=?1",
        params![lesson.session_id.parse::<i64>().unwrap()],
    )
    .unwrap();
    let next = language::start_session(&conn, "german", "2026-09-10", false).unwrap();
    assert_eq!(next.level, "B1");
}

#[test]
fn class_settings_stay_atomic_and_mutable_without_rewriting_the_accepted_path() {
    let (_, conn) = fixture();
    let request = setup(&conn, "german", "B1");
    let accepted = classes::accept(&conn, &request, "2026-09-09").unwrap();
    let mut input = classroom::ConfigureClassroomInput {
        subject_id: "german".into(),
        enabled: false,
        agent: "openrouter".into(),
        model: "openrouter/free".into(),
        custom_agent_bin: "".into(),
        session_minutes: 45,
        start_level: Some("B1".into()),
        target_level: Some("B2".into()),
        weekly_minutes: Some(315),
    };
    classroom::configure_program(&conn, &input, "2026-09-10").unwrap();
    let current = classes::current_configuration(&conn, "german")
        .unwrap()
        .unwrap();
    assert_eq!(current.tutor.provider, "openrouter");
    assert_eq!(current.pace.session_minutes, 45);
    assert_eq!(current.pace.weekly_minutes, Some(315));
    assert_eq!(
        conn.query_row("SELECT status FROM classes", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "paused"
    );
    assert_eq!(
        classes::current_path(&conn, "german")
            .unwrap()
            .unwrap()
            .configuration,
        accepted.configuration
    );
    input.agent = "codex".into();
    input.model = "default".into();
    input.weekly_minutes = Some(1);
    assert!(classroom::configure_program(&conn, &input, "2026-09-10").is_err());
    assert_eq!(
        classroom::program_row(&conn, "german").unwrap().agent,
        "openrouter"
    );
    assert_eq!(
        classes::current_configuration(&conn, "german")
            .unwrap()
            .unwrap(),
        current
    );
    // An accepted entry cannot be silently replaced through the legacy settings form.
    input.weekly_minutes = Some(315);
    input.start_level = Some("A1".into());
    assert!(classroom::configure_program(&conn, &input, "2026-09-10").is_err());
    assert_eq!(
        language::program_view(&conn, "german", "2026-09-10")
            .unwrap()
            .current_level,
        "B1"
    );
}

#[test]
fn diagnostic_acceptance_keeps_its_evidence_and_rejects_changed_setup() {
    use system_design_roulette_lib::domain::assessments::{Response, ResponseStatus};
    let (_, conn) = fixture();
    let options = enrollment::options("linux-bash").unwrap();
    let mut configuration = options.default_configuration;
    configuration.entry = EntryChoice::Diagnostic;
    let draft = enrollment::save_draft(
        &conn,
        &SaveEnrollmentDraft {
            id: None,
            expected_revision: None,
            course: options.course,
            configuration,
        },
    )
    .unwrap();
    let mut check = placement::start(&conn, &draft.id, draft.revision, false).unwrap();
    for question in &check.questions {
        check.revision = placement::save_response(
            &conn,
            &draft.id,
            &check.round_id,
            check.revision,
            &question.id,
            Response {
                answer: String::new(),
                status: ResponseStatus::Skipped,
            },
        )
        .unwrap();
    }
    placement::submit(&conn, &draft.id, &check.round_id, check.revision).unwrap();
    placement::finish(&conn, &draft.id, &check.round_id).unwrap();
    let recommendation = placement::recommend(&conn, &draft.id, draft.revision).unwrap();
    let input = AcceptPath {
        draft_id: draft.id.clone(),
        expected_revision: draft.revision,
        recommendation_id: recommendation.id,
    };
    let accepted = classes::accept(&conn, &input, "2026-09-09").unwrap();
    assert_eq!(
        accepted.recommendation.assessment_attempt_id,
        Some(check.attempt_id)
    );
    assert_eq!(accepted.recommendation.entry_point, "foundations");
    assert_eq!(count(&conn, "assessment_attempts"), 1);
    assert_eq!(count(&conn, "mastery"), 0);
    let next = setup(&conn, "linux-bash", "production");
    let mut draft = enrollment::draft(&conn, &next.draft_id).unwrap();
    draft.configuration.goal = LearningGoal::CourseOutcome {
        note: "A changed goal".into(),
    };
    enrollment::save_draft(
        &conn,
        &SaveEnrollmentDraft {
            id: Some(draft.id),
            expected_revision: Some(draft.revision),
            course: draft.course,
            configuration: draft.configuration,
        },
    )
    .unwrap();
    assert!(classes::accept(&conn, &next, "2026-09-10").is_err());
    assert_eq!(count(&conn, "path_revisions"), 1);
}

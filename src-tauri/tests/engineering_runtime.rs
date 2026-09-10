//! Engineering classes on the shared study runtime: planning on the accepted
//! path, frozen checks, saved work, transactional completion and the readers
//! that must switch together with the writer.
use rusqlite::{params, Connection};
use serde_json::json;
use system_design_roulette_lib::{
    classroom::{
        self, ConfigureClassroomInput, StoredEngineeringLesson, StoredQuestion,
        UpsertClassroomSlotInput,
    },
    db,
    domain::{
        assessments::{self, Owner, Purpose},
        classes, enrollment,
        sessions::{self, PreparedLesson, ReadingPosition, Session, Stage, Status},
    },
    language, mastery,
    progress::{self, ProgressQuery},
    subjects::engineering,
};

const TODAY: &str = "2026-07-21";

fn fixture() -> (std::path::PathBuf, Connection) {
    let dir = std::env::temp_dir().join(format!(
        "principia-engineering-{}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("learner.db");
    let conn = db::open(&path).unwrap();
    db::seed_concepts(&conn, include_str!("../seed/concepts.json")).unwrap();
    language::initialize(&conn, TODAY).unwrap();
    classroom::initialize(&conn).unwrap();
    (path, conn)
}

fn activate(conn: &Connection, subject_id: &str, hour: u32) -> i64 {
    let slot = classroom::upsert_slot(
        conn,
        &UpsertClassroomSlotInput {
            id: None,
            subject_id: subject_id.into(),
            hour,
            minute: 0,
            weekdays: vec![1, 2, 3, 4, 5, 6, 7],
            enabled: true,
        },
    )
    .unwrap();
    classroom::configure_program(
        conn,
        &ConfigureClassroomInput {
            subject_id: subject_id.into(),
            enabled: true,
            agent: "claude".into(),
            model: "sonnet".into(),
            custom_agent_bin: String::new(),
            session_minutes: 30,
            start_level: None,
            target_level: None,
            weekly_minutes: None,
        },
        TODAY,
    )
    .unwrap();
    slot
}

fn plan(conn: &Connection, subject_id: &str, slot: Option<i64>) -> Session {
    let program = classroom::program_row(conn, subject_id).unwrap();
    engineering::plan(conn, &program, slot, None, TODAY, false).unwrap()
}

fn lesson_for(session: &Session) -> StoredEngineeringLesson {
    let chosen = engineering::selection(session).unwrap();
    StoredEngineeringLesson {
        concept_id: chosen.concept_id,
        concept_title: chosen.title.clone(),
        category: chosen.category.clone(),
        title: format!("{} from first principles", chosen.title),
        markdown: "## The simple version\n\nA grounded fixture lesson.".into(),
        resources: vec![],
        questions: (1..=5)
            .map(|id| StoredQuestion {
                id,
                prompt: format!("Question {id}"),
                choices: vec![
                    "right".into(),
                    "wrong".into(),
                    "also wrong".into(),
                    "no".into(),
                ],
                correct_index: 0,
                explanation: format!("Because of mechanism {id}."),
                section: "Core mechanics".into(),
                learning_objective: format!("Objective {id}"),
            })
            .collect(),
        exercise: Some(system_design_roulette_lib::generator::Exercise {
            title: "Build the probe".into(),
            instructions: "Implement it.".into(),
            starter_code: Some("echo start".into()),
            deliverable: Some("A probe".into()),
            hints: vec![],
        }),
        source: "fixture".into(),
        path: None,
    }
}

fn publish(conn: &Connection, session: &Session) -> Session {
    let now = chrono::Utc::now();
    let lease = sessions::claim_preparation(conn, &session.id, now, 60)
        .unwrap()
        .unwrap();
    let stored = lesson_for(session);
    sessions::publish_preparation(
        conn,
        &lease,
        &PreparedLesson {
            title: stored.title.clone(),
            body: serde_json::to_value(&stored).unwrap(),
            provenance: json!({"kind":"fixture"}),
        },
        now,
    )
    .unwrap();
    let session = sessions::get(conn, &session.id).unwrap();
    engineering::ensure_check_round(conn, &session).unwrap();
    session
}

fn answer_all(conn: &Connection, session: &Session, wrong: &[usize]) -> classroom::CheckView {
    let view = engineering::view(conn, &session.id).unwrap().unwrap();
    let mut check = view.check.unwrap();
    for id in 1..=5 {
        let choice = if wrong.contains(&id) { 1 } else { 0 };
        check = engineering::save_answer(
            conn,
            &session.id,
            &check.round_id,
            check.revision,
            id,
            Some(choice),
        )
        .unwrap();
    }
    check
}

#[test]
fn activating_from_settings_plans_a_foundations_lesson_on_a_default_path() {
    let (_, conn) = fixture();
    activate(&conn, "javascript", 9);
    assert!(classes::current_path(&conn, "javascript")
        .unwrap()
        .is_none());
    let session = plan(&conn, "javascript", None);
    let path = classes::current_path(&conn, "javascript").unwrap().unwrap();
    assert_eq!(path.revision, 1);
    assert_eq!(path.recommendation.route, "foundations");
    assert_eq!(path.configuration.tutor.model, "sonnet");
    assert_eq!(session.status, Status::Planned);
    let chosen = engineering::selection(&session).unwrap();
    assert_eq!(chosen.adapter, "engineering");
    assert_eq!(chosen.service_date, TODAY);
    // Planning again resumes the same saved session instead of drawing twice.
    assert_eq!(plan(&conn, "javascript", None).id, session.id);
    assert_eq!(
        engineering::active_summaries(&conn).unwrap()[0].lifecycle,
        "planned"
    );
    assert!(engineering::view(&conn, &session.id).unwrap().is_none());
}

#[test]
fn an_unfinished_starting_point_draft_blocks_the_default_path() {
    let (_, conn) = fixture();
    activate(&conn, "typescript", 10);
    let options = enrollment::options("typescript").unwrap();
    enrollment::save_draft(
        &conn,
        &enrollment::SaveEnrollmentDraft {
            id: None,
            expected_revision: None,
            course: options.course,
            configuration: options.default_configuration,
        },
    )
    .unwrap();
    let program = classroom::program_row(&conn, "typescript").unwrap();
    let error = engineering::plan(&conn, &program, None, None, TODAY, false).unwrap_err();
    assert!(error.contains("starting point"), "{error}");
    assert!(classes::current_path(&conn, "typescript")
        .unwrap()
        .is_none());
}

#[test]
fn a_prepared_lesson_freezes_its_check_and_saved_answers_survive_reopen() {
    let (path, conn) = fixture();
    let slot = activate(&conn, "linux-bash", 8);
    let planned = plan(&conn, "linux-bash", Some(slot));
    let ready = publish(&conn, &planned);
    assert_eq!(ready.status, Status::Ready);
    let view = engineering::view(&conn, &ready.id).unwrap().unwrap();
    assert_eq!(view.runtime, "study");
    assert_eq!(view.lifecycle, "ready");
    assert_eq!(view.status, "in_progress");
    assert_eq!(view.questions.len(), 5);
    let check = view.check.clone().unwrap();
    assert_eq!(check.revision, 0);
    assert!(!check.submitted);
    // The displayed round is frozen: a second view never resamples it.
    assert_eq!(
        engineering::view(&conn, &ready.id)
            .unwrap()
            .unwrap()
            .check
            .unwrap()
            .round_id,
        check.round_id
    );
    let active = engineering::activate(&conn, &ready.id).unwrap();
    assert_eq!(active.status, Status::Active);
    let saved = engineering::save_answer(&conn, &ready.id, &check.round_id, 0, 1, Some(0)).unwrap();
    assert_eq!(saved.revision, 1);
    let stale =
        engineering::save_answer(&conn, &ready.id, &check.round_id, 0, 2, Some(1)).unwrap_err();
    assert!(stale.contains("changed"), "{stale}");
    assert!(engineering::save_answer(&conn, &ready.id, &check.round_id, 1, 2, Some(9)).is_err());
    let checkpoint = engineering::patch_work(
        &conn,
        &ready.id,
        0,
        Some(Stage::Practice),
        Some(ReadingPosition {
            anchor: Some("#core".into()),
            offset: 42,
        }),
        [("exercise_draft".to_string(), json!("echo probe"))]
            .into_iter()
            .collect(),
    )
    .unwrap();
    assert_eq!(checkpoint.revision, 1);
    drop(conn);
    let conn = db::open(&path).unwrap();
    let reopened = engineering::view(&conn, &ready.id).unwrap().unwrap();
    assert_eq!(reopened.lifecycle, "active");
    let check = reopened.check.unwrap();
    assert_eq!(check.revision, 1);
    assert_eq!(check.responses["1"].answer, "0");
    let saved = reopened.checkpoint.unwrap();
    assert_eq!(saved.body.stage, Stage::Practice);
    assert_eq!(saved.body.reading.offset, 42);
    assert_eq!(saved.body.work["exercise_draft"], json!("echo probe"));
    let exercise = engineering::exercise_view(&conn, &ready.id)
        .unwrap()
        .unwrap();
    assert_eq!(exercise.draft.as_deref(), Some("echo probe"));
    assert_eq!(
        exercise.study_session_id.as_deref(),
        Some(ready.id.0.as_str())
    );
    // The appointment is consumed for the day by the planned session.
    assert!(classroom::validate_slot_start(&conn, "linux-bash", slot, TODAY).is_err());
    let slots = classroom::slot_views(&conn, TODAY, true).unwrap();
    let mine = slots.iter().find(|s| s.id == slot).unwrap();
    assert!(mine.in_progress && !mine.owed);
}

#[test]
fn submission_grades_the_displayed_round_transactionally_and_repeats_safely() {
    let (_, conn) = fixture();
    activate(&conn, "javascript", 9);
    let session = publish(&conn, &plan(&conn, "javascript", None));
    engineering::activate(&conn, &session.id).unwrap();
    let check = answer_all(&conn, &session, &[2, 4]);
    let concept_id = engineering::selection(&session).unwrap().concept_id;
    let result = engineering::submit(
        &conn,
        &session.id,
        &check.round_id,
        check.revision,
        "I traced the mechanism.",
        TODAY,
    )
    .unwrap();
    assert_eq!(result.session_id, session.id.0);
    assert!((result.score - 0.6).abs() < 1e-9);
    assert!(!result.passed);
    assert_eq!(result.corrections.iter().filter(|c| !c.correct).count(), 2);
    let evidence: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM classroom_exit_attempts WHERE study_session_id=?1 AND concept_id=?2",
            params![session.id.0, concept_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(evidence, 5);
    assert_eq!(mastery::get(&conn, concept_id).unwrap().encounters, 1);
    let finished = sessions::get(&conn, &session.id).unwrap();
    assert_eq!(finished.status, Status::Completed);
    assert_eq!(finished.checkpoint.body.stage, Stage::Feedback);
    assert_eq!(
        finished.checkpoint.body.work["reflection"],
        json!("I traced the mechanism.")
    );
    let attempt = assessments::latest(
        &conn,
        &Owner::StudySession(session.id.0.clone()),
        Purpose::ExitCheck,
    )
    .unwrap()
    .unwrap();
    assert_eq!(attempt.status, "completed");
    assert!(attempt.rounds[0].submission.is_some());
    // Repeating the submission returns the original result without new evidence.
    let again = engineering::submit(
        &conn,
        &session.id,
        &check.round_id,
        check.revision,
        "changed",
        TODAY,
    )
    .unwrap();
    assert!((again.score - result.score).abs() < 1e-9);
    let evidence_again: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM classroom_exit_attempts WHERE study_session_id=?1",
            [&session.id.0],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(evidence_again, 5);
    assert!(engineering::save_answer(
        &conn,
        &session.id,
        &check.round_id,
        check.revision,
        1,
        Some(1)
    )
    .is_err());
    let view = engineering::view(&conn, &session.id).unwrap().unwrap();
    assert_eq!(view.status, "completed");
    assert!(view.outcome.unwrap().corrections.len() == 5);
    assert_eq!(
        classroom::completed_lesson_count(&conn, "javascript").unwrap(),
        1
    );
    assert!(engineering::active_summaries(&conn).unwrap().is_empty());
    let dashboard = progress::read(&conn, TODAY, &ProgressQuery::default()).unwrap();
    let entry = dashboard
        .history
        .iter()
        .find(|e| e.source == "study")
        .unwrap();
    assert_eq!(
        (
            entry.status.as_str(),
            entry.subject_id.as_str(),
            entry.can_read
        ),
        ("completed", "javascript", true)
    );
    assert!((entry.score.unwrap() - 0.6).abs() < 1e-9);
    assert_eq!(dashboard.completed_sessions, 1);
    let archived = progress::lesson(&conn, "study", &session.id.0)
        .unwrap()
        .unwrap();
    assert!(archived.markdown.contains("fixture lesson"));
    assert_eq!(
        archived.study_session_id.as_deref(),
        Some(session.id.0.as_str())
    );
    let dossier = mastery::build_dossier(&conn, "2026-07-22", "javascript").unwrap();
    assert!(dossier.contains("Day 2 of teaching"), "{dossier}");
    assert!(dossier.contains("Objective 2"), "{dossier}");
    // The next lesson moves on: a new session with a different topic selection.
    let next = plan(&conn, "javascript", None);
    assert_ne!(next.id, session.id);
    assert_ne!(
        engineering::selection(&next).unwrap().concept_id,
        concept_id
    );
}

#[test]
fn unanswered_checks_cannot_be_submitted_and_skipping_keeps_work_readable() {
    let (_, conn) = fixture();
    activate(&conn, "typescript", 11);
    let session = publish(&conn, &plan(&conn, "typescript", None));
    engineering::activate(&conn, &session.id).unwrap();
    let check = engineering::save_answer(
        &conn,
        &session.id,
        &engineering::view(&conn, &session.id)
            .unwrap()
            .unwrap()
            .check
            .unwrap()
            .round_id,
        0,
        1,
        Some(0),
    )
    .unwrap();
    let error = engineering::submit(
        &conn,
        &session.id,
        &check.round_id,
        check.revision,
        "",
        TODAY,
    )
    .unwrap_err();
    assert!(error.contains("Answer every"), "{error}");
    assert_eq!(
        sessions::get(&conn, &session.id).unwrap().status,
        Status::Active
    );
    let skipped = engineering::skip(&conn, &session.id).unwrap();
    assert_eq!(skipped.status, Status::Skipped);
    assert!(engineering::submit(
        &conn,
        &session.id,
        &check.round_id,
        check.revision,
        "",
        TODAY
    )
    .is_err());
    let view = engineering::view(&conn, &session.id).unwrap().unwrap();
    assert_eq!(view.status, "skipped");
    assert!(view.outcome.is_none());
    assert!(view.markdown.contains("fixture lesson"));
    assert_eq!(
        classroom::completed_lesson_count(&conn, "typescript").unwrap(),
        0
    );
    let history = progress::read(&conn, TODAY, &ProgressQuery::default())
        .unwrap()
        .history;
    assert_eq!(
        history.iter().find(|e| e.source == "study").unwrap().status,
        "skipped"
    );
    assert_ne!(plan(&conn, "typescript", None).id, session.id);
}

#[test]
fn a_failed_preparation_keeps_the_request_visible_for_retry() {
    let (_, conn) = fixture();
    activate(&conn, "frontend-architecture", 12);
    let session = plan(&conn, "frontend-architecture", None);
    let now = chrono::Utc::now();
    let lease = sessions::claim_preparation(&conn, &session.id, now, 60)
        .unwrap()
        .unwrap();
    sessions::fail_preparation(&conn, &lease, "provider unavailable", now).unwrap();
    let summary = &engineering::active_summaries(&conn).unwrap()[0];
    assert_eq!(summary.lifecycle, "preparing");
    assert!(classroom::active_sessions(&conn)
        .unwrap()
        .iter()
        .any(|s| s.session_id == session.id.0 && s.runtime == "study"));
    assert_eq!(
        sessions::preparation(&conn, &session.id)
            .unwrap()
            .error
            .as_deref(),
        Some("provider unavailable")
    );
    assert!(engineering::activate(&conn, &session.id).is_err());
    sessions::retry_preparation(&conn, &session.id, now).unwrap();
    let published = publish(&conn, &session);
    assert_eq!(published.status, Status::Ready);
}

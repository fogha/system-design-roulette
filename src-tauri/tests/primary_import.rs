use rusqlite::{params, Connection, OpenFlags};
use serde_json::json;
use std::path::PathBuf;
use system_design_roulette_lib::{
    db,
    domain::assessments::{self, Item, Owner, Purpose, Response, ResponseStatus},
    storage::primary_import::{self, Cell, ContentSelection, RecoveryReason, SubjectResolution},
};

fn fixture() -> (PathBuf, Connection) {
    let directory = std::env::temp_dir().join(format!(
        "principia-primary-import-{:032x}",
        rand::random::<u128>()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("learner.db");
    let conn = db::open(&path).unwrap();
    conn.execute_batch("INSERT INTO concepts(id,slug,title,category,focus,brief_json) VALUES
        (1,'old-js','Original JS topic','fundamentals','javascript','{ \"old\": true }'),
        (2,'old-design','Pre-drawn topic','fundamentals','system-design','{}');
        INSERT INTO sessions(date,concept_id,status,current_step,reading_seconds,focus) VALUES
        ('2026-09-01',1,'in_progress','course',17,'javascript'),
        ('2026-09-02',1,'in_progress','review',0,'javascript'),
        ('2026-09-03',2,'pending','quiz',0,''),
        ('2026-09-04',1,'in_progress','course',9,'typescript');
        INSERT INTO sessions(date,concept_id,status,current_step,reading_seconds,focus,completed_at) VALUES
        ('2026-08-31',1,'completed','done',1800,'javascript','2026-08-31T10:00:00');
        INSERT INTO courses(id,session_date,concept_id,markdown,resources_json,source,generated_at) VALUES
        (11,'2026-09-01',1,'# Original\n\nExact lesson body.','[  ]','fallback','original'),
        (12,'2026-09-02',1,'# Candidate one','[]','claude','first'),
        (13,'2026-09-02',1,'# Candidate two','[]','codex','second'),
        (14,'2026-09-03',2,'# A pre-drawn document is not a subject choice','[]','fallback','old'),
        (15,'2026-09-04',1,'# Subject mismatch','[]','fallback','old'),
        (16,'2026-08-31',1,'# Archived completed lesson','[]','claude','original'),
        (17,'2026-08-30',1,'# Orphan archive document','[]','fallback','original');
        INSERT INTO course_exercises VALUES(11,'Saved practical work','Original instructions',NULL,NULL,'[ \"first\", \"second\" ]');
        INSERT INTO exercise_drafts(course_id,draft,completed,reflection,updated_at) VALUES(11,'const unfinished = ',0,'  Original reflection  ','original');
        INSERT INTO config VALUES('provider_api_key','PRIVATE_TOKEN_MUST_NOT_LEAK');
        INSERT INTO config VALUES('planned:2026-09-01','1');
        INSERT INTO config VALUES('exit_quiz_round:11','3');
        INSERT INTO config VALUES('exit_quiz_count:11','9');
        INSERT INTO profile VALUES('preferred_focus','typescript');
        INSERT INTO legacy_assessment_config VALUES('quiz_round:2026-09-03','{malformed original');
        INSERT INTO legacy_assessment_config VALUES('pending_answers:2026-09-03','{ \"41\": \"original draft\" }');
        INSERT INTO generation_jobs(kind,target_date,status,attempts,created_at) VALUES('course','2026-09-03','running',2,'original');").unwrap();
    let question = db::insert_question(
        &conn,
        11,
        "Original frozen question",
        "free",
        None,
        "original rubric",
        "original explanation",
    )
    .unwrap();
    let stored_question = db::questions_for_course(&conn, 11).unwrap().remove(0);
    let owner = Owner::LegacyPrimary("2026-09-02".into());
    let round = assessments::start(
        &conn,
        &owner,
        Purpose::Retrieval,
        &json!({"stored_context":true}),
        "original-rubric",
        &[Item {
            id: question.to_string(),
            body: serde_json::to_value(stored_question).unwrap(),
        }],
    )
    .unwrap();
    assessments::save_response(
        &conn,
        &owner,
        &round.id,
        0,
        &question.to_string(),
        Response {
            answer: "The learner's unfinished answer…".into(),
            status: ResponseStatus::Draft,
        },
    )
    .unwrap();
    (path, conn)
}

#[test]
fn import_preserves_every_legacy_owner_without_inventing_assignment_or_credit() {
    let (_path, conn) = fixture();
    let before_changes = conn.total_changes();
    let plan = primary_import::inspect(&conn).unwrap();
    assert_eq!(
        conn.total_changes(),
        before_changes,
        "inspection must not write"
    );
    assert_eq!(plan.sessions.len(), 5);
    assert_eq!(
        plan.sessions
            .iter()
            .filter(|s| s.original_status == "in_progress")
            .count(),
        3
    );
    let reading = plan
        .sessions
        .iter()
        .find(|s| s.service_date == "2026-09-01")
        .unwrap();
    assert_eq!(
        reading.session_id,
        db::primary_session_id(&conn, &reading.service_date)
            .unwrap()
            .unwrap()
    );
    assert_eq!(reading.content, ContentSelection::Stored { course_id: 11 });
    assert!(reading.recovery.is_empty());
    let pending = plan
        .sessions
        .iter()
        .find(|s| s.service_date == "2026-09-03")
        .unwrap();
    assert_eq!(
        pending.subject,
        SubjectResolution::NotSelected,
        "neither a pre-drawn concept nor preferred focus chooses the session subject"
    );
    assert_eq!(pending.original_status, "pending");
    assert_eq!(
        pending.content,
        ContentSelection::Ambiguous {
            course_ids: vec![14]
        }
    );
    assert!(pending
        .recovery
        .contains(&RecoveryReason::SubjectNotRecorded));
    assert!(pending
        .recovery
        .contains(&RecoveryReason::AssessmentFormatUnknown));
    let conflicting = plan
        .sessions
        .iter()
        .find(|s| s.service_date == "2026-09-04")
        .unwrap();
    assert_eq!(
        conflicting.subject,
        SubjectResolution::Recorded {
            course_id: "typescript".into()
        }
    );
    assert!(conflicting
        .recovery
        .contains(&RecoveryReason::SubjectConceptMismatch));
    assert_eq!(plan.unattached_course_ids, vec![17]);
    for table in ["study_sessions", "classes", "path_revisions", "mastery"] {
        let count: u32 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "inspection created records in {table}");
    }
}

#[test]
fn duplicate_documents_and_unknown_payloads_are_retained_byte_for_byte() {
    let (path, conn) = fixture();
    // SQLite TEXT can contain non-UTF8 bytes even though Rust Strings cannot.
    conn.execute(
        "UPDATE courses SET resources_json=CAST(?1 AS TEXT) WHERE id=12",
        [vec![0xffu8, 0, 0xc0]],
    )
    .unwrap();
    let original_draft: String = conn
        .query_row(
            "SELECT draft FROM exercise_drafts WHERE course_id=11",
            [],
            |r| r.get(0),
        )
        .unwrap();
    drop(conn);
    let reader = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let plan = primary_import::inspect(&reader).unwrap();
    let round_owner = plan
        .sessions
        .iter()
        .find(|s| s.service_date == "2026-09-02")
        .unwrap();
    assert_eq!(
        round_owner.content,
        ContentSelection::Ambiguous {
            course_ids: vec![12, 13]
        }
    );
    assert_eq!(round_owner.assessment_attempt_ids.len(), 1);
    let invalid = plan.tables["courses"]
        .iter()
        .find(|r| r["id"] == Cell::Integer(12))
        .unwrap();
    assert_eq!(invalid["resources_json"], Cell::Text(vec![0xff, 0, 0xc0]));
    assert_eq!(
        plan.tables["exercise_drafts"][0]["draft"],
        Cell::Text(original_draft.into_bytes())
    );
    assert_eq!(
        plan.tables["legacy_assessment_config"][1]["value"],
        Cell::Text(b"{malformed original".to_vec())
    );
    assert_eq!(plan.tables["assessment_work"].len(), 1);
    assert_eq!(plan.tables["primary_config"].len(), 3);
    let serialized = serde_json::to_string(&plan).unwrap();
    assert!(!serialized.contains("PRIVATE_TOKEN_MUST_NOT_LEAK"));
    assert!(!plan.tables.contains_key("profile"));
    assert_eq!(
        serde_json::from_str::<primary_import::PrimaryImport>(&serialized).unwrap(),
        plan
    );
    primary_import::verify_unchanged(&reader, &plan).unwrap();
}

#[test]
fn a_primary_write_invalidates_the_reviewed_import_but_other_class_work_does_not() {
    let (_path, conn) = fixture();
    let original = primary_import::inspect(&conn).unwrap();
    // Configuration changes must not rewrite historical tutor provenance.
    db::set_config(&conn, "agent", "ollama").unwrap();
    db::set_config(&conn, "model", "a-new-default").unwrap();
    conn.execute_batch("INSERT INTO classroom_programs(subject_id,kind,label,short_code,enabled,agent,model,prompt_profile,updated_at) VALUES('javascript','engineering','A separate class','JS',1,'claude','sonnet','javascript','original');
        INSERT INTO classroom_sessions(id,subject_id,session_date,status,title,payload_json,agent_used,prompt_version,started_at) VALUES(101,'javascript','2026-09-01','in_progress','Another class','{}','fallback','legacy','original');
        INSERT INTO exercise_drafts(classroom_session_id,draft,completed,reflection,updated_at) VALUES(101,'Another class draft',0,'','original');").unwrap();
    db::save_exercise_draft(&conn, None, Some(101), "A later class draft").unwrap();
    primary_import::verify_unchanged(&conn, &original).unwrap();
    let id = db::primary_session_id(&conn, "2026-09-01")
        .unwrap()
        .unwrap();
    db::save_primary_reading(&conn, &id, 18).unwrap();
    assert!(primary_import::verify_unchanged(&conn, &original).is_err());
    let tick = primary_import::inspect(&conn).unwrap();
    assert_ne!(tick.fingerprint, original.fingerprint);
    db::save_exercise_draft(&conn, Some(11), None, "A later durable draft").unwrap();
    assert!(primary_import::verify_unchanged(&conn, &tick).is_err());
    let draft = primary_import::inspect(&conn).unwrap();
    db::insert_course(&conn, "2026-09-01", 1, "# Late worker", "[]", "claude").unwrap();
    assert!(primary_import::verify_unchanged(&conn, &draft).is_err());
}

#[test]
fn inspection_uses_one_sqlite_snapshot_and_detects_drift_after_it_closes() {
    let (path, writer) = fixture();
    let reader = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let snapshot = reader.unchecked_transaction().unwrap();
    let original = primary_import::inspect(&snapshot).unwrap();
    writer
        .execute(
            "UPDATE courses SET markdown='new worker publication' WHERE id=11",
            [],
        )
        .unwrap();
    primary_import::verify_unchanged(&snapshot, &original).unwrap();
    snapshot.commit().unwrap();
    assert!(primary_import::verify_unchanged(&reader, &original).is_err());
}

#[test]
fn inconsistent_crosswalks_and_unmapped_identities_stop_import() {
    let (_path, conn) = fixture();
    let expected = db::primary_session_id(&conn, "2026-09-01")
        .unwrap()
        .unwrap();
    conn.execute("UPDATE legacy_crosswalk SET entity_id='wrong-owner' WHERE legacy_table='sessions' AND legacy_key='2026-09-01'", []).unwrap();
    assert!(primary_import::inspect(&conn)
        .unwrap_err()
        .to_string()
        .contains("crosswalk"));
    conn.execute("UPDATE legacy_crosswalk SET entity_id=?1 WHERE legacy_table='sessions' AND legacy_key='2026-09-01'", [&expected]).unwrap();
    conn.execute("INSERT INTO legacy_crosswalk VALUES('sessions','missing-row','primary_session','unmapped','old')", []).unwrap();
    assert!(primary_import::inspect(&conn).is_err());
}

#[test]
fn unknown_stages_missing_content_and_missing_terminal_times_need_recovery() {
    let (_path, conn) = fixture();
    conn.execute("INSERT INTO sessions(date,status,current_step,reading_seconds,focus,session_type) VALUES('not-a-date','in_progress','unknown-stage',-4,'javascript','unknown-kind')", []).unwrap();
    conn.execute("INSERT INTO sessions(date,status,current_step,focus) VALUES('2026-09-05','in_progress','course','javascript')", []).unwrap();
    conn.execute(
        "UPDATE sessions SET completed_at=NULL WHERE date=?1",
        params!["2026-08-31"],
    )
    .unwrap();
    let plan = primary_import::inspect(&conn).unwrap();
    let unusual = plan
        .sessions
        .iter()
        .find(|s| s.service_date == "not-a-date")
        .unwrap();
    for reason in [
        RecoveryReason::InvalidServiceDate,
        RecoveryReason::UnknownStage,
        RecoveryReason::UnknownSessionKind,
        RecoveryReason::InvalidReadingTime,
    ] {
        assert!(unusual.recovery.contains(&reason));
    }
    assert!(plan
        .sessions
        .iter()
        .find(|s| s.service_date == "2026-09-05")
        .unwrap()
        .recovery
        .contains(&RecoveryReason::ReadingWithoutLesson));
    let finished = plan
        .sessions
        .iter()
        .find(|s| s.service_date == "2026-08-31")
        .unwrap();
    assert_eq!(finished.original_status, "completed");
    assert!(finished
        .recovery
        .contains(&RecoveryReason::TerminalTimeNotRecorded));
}

#[test]
fn inspection_rejects_an_unknown_schema_or_modified_migration_ledger() {
    let (_path, conn) = fixture();
    conn.pragma_update(None, "user_version", 99).unwrap();
    assert!(primary_import::inspect(&conn)
        .unwrap_err()
        .to_string()
        .contains("schema v7"));
    conn.pragma_update(None, "user_version", 7).unwrap();
    conn.execute(
        "UPDATE schema_migrations SET checksum='tampered' WHERE version=6",
        [],
    )
    .unwrap();
    assert!(primary_import::inspect(&conn)
        .unwrap_err()
        .to_string()
        .contains("migration history mismatch"));
}

#[test]
fn unfamiliar_frozen_rounds_and_lesson_metadata_require_recovery_without_resampling() {
    let (_path, conn) = fixture();
    let owner = Owner::LegacyPrimary("2026-09-01".into());
    assessments::start(
        &conn,
        &owner,
        Purpose::Retrieval,
        &json!({"old":true}),
        "unknown-old-format",
        &[Item {
            id: "old-question".into(),
            body: json!({"legacy_prompt":"must remain intact"}),
        }],
    )
    .unwrap();
    conn.execute(
        "UPDATE courses SET resources_json='{ malformed stored sources' WHERE id=11",
        [],
    )
    .unwrap();
    let original_questions: i64 = conn
        .query_row("SELECT COUNT(*) FROM questions", [], |r| r.get(0))
        .unwrap();
    let plan = primary_import::inspect(&conn).unwrap();
    let row = plan
        .sessions
        .iter()
        .find(|s| s.service_date == "2026-09-01")
        .unwrap();
    assert!(row
        .recovery
        .contains(&RecoveryReason::AssessmentFormatUnknown));
    assert!(row
        .recovery
        .contains(&RecoveryReason::LessonMetadataUnknown));
    assert_eq!(row.content, ContentSelection::Stored { course_id: 11 });
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM questions", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        original_questions
    );
}

#[test]
fn original_main_and_pr_upgrade_records_keep_system_design_and_all_archived_documents() {
    for schema in [
        include_str!("fixtures/upgrades/original-main.sql"),
        include_str!("fixtures/upgrades/pr-head.sql"),
        include_str!("fixtures/upgrades/classroom-exercises.sql"),
    ] {
        let directory = std::env::temp_dir().join(format!(
            "principia-primary-old-{:032x}",
            rand::random::<u128>()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("learner.db");
        let old = Connection::open(&path).unwrap();
        old.execute_batch(schema).unwrap();
        old.execute_batch(include_str!("fixtures/upgrades/common-records.sql"))
            .unwrap();
        drop(old);
        let conn = db::open(&path).unwrap();
        let plan = primary_import::inspect(&conn).unwrap();
        assert_eq!(plan.sessions.len(), 1);
        assert_eq!(
            plan.sessions[0].subject,
            SubjectResolution::LinkedConcept {
                course_id: "system-design".into(),
                concept_id: 50
            }
        );
        assert_eq!(plan.sessions[0].original_status, "in_progress");
        assert_eq!(
            plan.sessions[0].content,
            ContentSelection::Ambiguous {
                course_ids: vec![501, 502]
            }
        );
        assert_eq!(
            plan.tables["audio_scripts"][0]["audio_dir"],
            Cell::Text(b"/retained/audio/501".to_vec())
        );
        assert_eq!(
            plan.tables["attempts"][0]["user_answer"],
            Cell::Text(b"A".to_vec())
        );
        let progress: (String, f64, i64) = conn
            .query_row(
                "SELECT state,score_ema,encounters FROM mastery WHERE concept_id=50",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(progress, ("practicing".into(), 0.725, 4));
        assert_eq!(
            plan.tables["sessions"][0]["focus"],
            Cell::Text(Vec::new()),
            "subject recovery must not rewrite the legacy source"
        );
    }
}

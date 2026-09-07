use system_design_roulette_lib::db::{self, Session};
use system_design_roulette_lib::language::{
    self, ConfigureProgramInput, SubmitSessionInput, UpsertSlotInput,
};

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn test_db() -> rusqlite::Connection {
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("sdr-language-test-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let conn = db::open(&dir.join("test.db")).unwrap();
    language::initialize(&conn, "2026-07-21").unwrap();
    db::set_config(&conn, "schedule_hour", "19").unwrap();
    db::set_config(&conn, "schedule_minute", "0").unwrap();
    conn
}

fn correct_answers(conn: &rusqlite::Connection, session_id: i64) -> Vec<usize> {
    let lesson_json: String = conn
        .query_row(
            "SELECT lesson_json FROM language_sessions WHERE id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .unwrap();
    let payload: serde_json::Value = serde_json::from_str(&lesson_json).unwrap();
    payload["questions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|question| question["correct_index"].as_u64().unwrap() as usize)
        .collect()
}

fn enable_german(conn: &rusqlite::Connection) {
    language::configure_program(
        conn,
        &ConfigureProgramInput {
            language: "german".into(),
            enabled: true,
            start_level: "A1".into(),
            target_level: "A2".into(),
            weekly_minutes: 210,
            session_minutes: 30,
        },
        "2026-07-21",
    )
    .unwrap();
}

fn pass_session(conn: &rusqlite::Connection, session_id: i64, today: &str) {
    let result = language::submit_session(
        conn,
        &SubmitSessionInput {
            session_id,
            answers: correct_answers(conn, session_id),
            writing_response:
                "I completed the requested writing practice with enough detail to show my work."
                    .into(),
            speaking_completed: true,
            listened: true,
            confidence: 4,
        },
        today,
    )
    .unwrap();
    assert!(result.passed);
}

#[test]
fn bundled_language_curricula_meet_the_quality_contract() {
    language::validate_curriculum("german").unwrap();
    language::validate_curriculum("italian").unwrap();
    assert!(language::validate_curriculum("french").is_err());
    assert!(language::pedagogical_summary("german")
        .unwrap()
        .contains("40 action-oriented CEFR units"));
    assert!(language::pedagogical_summary("italian")
        .unwrap()
        .contains("starts from first principles"));
}

#[test]
fn a1_completes_alphabet_and_counting_foundations_before_later_scenarios() {
    let conn = test_db();
    enable_german(&conn);

    for day in 21..=23 {
        let today = format!("2026-07-{day}");
        let lesson = language::start_session(&conn, "german", None, &today).unwrap();
        assert_eq!(lesson.unit_slug, "de-a1-greetings-introductions");
        assert!(lesson.markdown.contains("## First principles foundation"));
        assert!(lesson
            .markdown
            .contains("The German alphabet and letter names"));
        assert!(lesson.markdown.contains("A ah, B beh, C tseh"));
        pass_session(&conn, lesson.session_id, &today);
    }

    let numbers = language::start_session(&conn, "german", None, "2026-07-24").unwrap();
    assert_eq!(numbers.unit_slug, "de-a1-numbers-personal-info");
    assert!(numbers
        .markdown
        .contains("Count from zero before using numbers"));
    assert!(numbers
        .markdown
        .contains("null, eins, zwei, drei, vier, fünf"));
}

#[test]
fn italian_a1_starts_with_alphabet_and_sound_spelling() {
    let conn = test_db();
    language::configure_program(
        &conn,
        &ConfigureProgramInput {
            language: "italian".into(),
            enabled: true,
            start_level: "A1".into(),
            target_level: "A2".into(),
            weekly_minutes: 210,
            session_minutes: 30,
        },
        "2026-07-21",
    )
    .unwrap();

    let lesson = language::start_session(&conn, "italian", None, "2026-07-21").unwrap();
    assert_eq!(lesson.unit_slug, "it-a1-greetings");
    assert!(lesson
        .markdown
        .contains("The Italian alphabet and letter names"));
    assert!(lesson.markdown.contains("Core Italian uses 21 letters"));
    assert!(lesson.markdown.contains("Vowels are the sound anchors"));
}

#[test]
fn opening_an_existing_database_adds_language_tables_without_touching_old_data() {
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "sdr-language-migration-{}-{}",
        std::process::id(),
        n
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.db");
    {
        let old = rusqlite::Connection::open(&path).unwrap();
        old.execute_batch(
            "CREATE TABLE config (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO config (key, value) VALUES ('legacy-marker', 'preserved');",
        )
        .unwrap();
    }
    let conn = db::open(&path).unwrap();
    language::initialize(&conn, "2026-07-21").unwrap();
    let marker = db::get_config(&conn, "legacy-marker").unwrap();
    assert_eq!(marker.as_deref(), Some("preserved"));
    let language_tables: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name LIKE 'language_%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(language_tables, 5);
}

#[test]
fn language_slots_extend_the_os_schedule_without_replacing_frontend_time() {
    let conn = test_db();
    enable_german(&conn);
    for (hour, minute) in [(7, 30), (12, 15)] {
        language::upsert_slot(
            &conn,
            &UpsertSlotInput {
                id: None,
                language: "german".into(),
                hour,
                minute,
                weekdays: vec![1, 2, 3, 4, 5, 6],
                enabled: true,
            },
        )
        .unwrap();
    }
    assert_eq!(
        language::all_schedule_times(&conn).unwrap(),
        vec![(7, 30), (12, 15), (19, 0)]
    );
}

#[test]
fn frontend_completion_does_not_consume_or_mutate_language_practice() {
    let conn = test_db();
    enable_german(&conn);
    let slot_id = language::upsert_slot(
        &conn,
        &UpsertSlotInput {
            id: None,
            language: "german".into(),
            hour: 7,
            minute: 30,
            weekdays: vec![1, 2, 3, 4, 5, 6, 7],
            enabled: true,
        },
    )
    .unwrap();
    let frontend = Session {
        date: "2026-07-21".into(),
        concept_id: None,
        status: "completed".into(),
        current_step: "done".into(),
        quiz_score: Some(1.0),
        started_at: Some("2026-07-21T19:00:00".into()),
        completed_at: Some("2026-07-21T19:30:00".into()),
        reading_seconds: 0,
        session_type: "lesson".into(),
        plan_reason: String::new(),
        focus: "javascript".into(),
    };
    db::upsert_session(&conn, &frontend).unwrap();

    let slots = language::slot_views(&conn, "2026-07-21", true).unwrap();
    assert!(slots.iter().any(|slot| slot.id == slot_id && slot.owed));

    let lesson = language::start_session(&conn, "german", Some(slot_id), "2026-07-21").unwrap();
    assert_eq!(lesson.language, "german");
    assert_eq!(lesson.level, "A1");
    assert_eq!(lesson.unit_slug, "de-a1-greetings-introductions");
    assert_eq!(lesson.questions.len(), 5);
    assert!(lesson.markdown.contains("## Model dialogue"));

    let unchanged = db::get_session(&conn, "2026-07-21").unwrap().unwrap();
    assert_eq!(unchanged.status, "completed");
    assert_eq!(unchanged.focus, "javascript");
}

#[test]
fn passed_language_session_records_skill_and_unit_evidence() {
    let conn = test_db();
    enable_german(&conn);
    let lesson = language::start_session(&conn, "german", None, "2026-07-21").unwrap();
    let answers = correct_answers(&conn, lesson.session_id);
    let result = language::submit_session(
        &conn,
        &SubmitSessionInput {
            session_id: lesson.session_id,
            answers,
            writing_response:
                "Guten Tag. Ich heiße Armand und ich komme aus England. Freut mich sehr.".into(),
            speaking_completed: true,
            listened: true,
            confidence: 4,
        },
        "2026-07-21",
    )
    .unwrap();
    assert!(result.passed);
    assert!(result.score >= 0.8);
    assert_eq!(result.progress.completed_steps, 1);
    assert!(result
        .progress
        .skills
        .iter()
        .any(|skill| skill.id == "listening" && skill.encounters > 0));
    assert!(language::active_session(&conn).unwrap().is_none());
}

#[test]
fn cefr_level_advances_only_after_unit_and_skill_gate_evidence() {
    let conn = test_db();
    enable_german(&conn);
    let seed: serde_json::Value =
        serde_json::from_str(include_str!("../seed/languages/german.json")).unwrap();
    for unit in seed["levels"][0]["units"].as_array().unwrap() {
        let slug = unit["slug"].as_str().unwrap();
        conn.execute(
            "INSERT INTO language_unit_progress
                (language, unit_slug, phase_completed, score_ema, encounters)
             VALUES ('german', ?1, 3, 0.9, 3)",
            [slug],
        )
        .unwrap();
    }
    for strand in language::STRANDS {
        conn.execute(
            "INSERT INTO language_skill_scores
                (language, strand, score_ema, encounters)
             VALUES ('german', ?1, 0.8, 4)",
            [strand],
        )
        .unwrap();
    }
    let lesson = language::start_session(&conn, "german", None, "2026-07-21").unwrap();
    let result = language::submit_session(
        &conn,
        &SubmitSessionInput {
            session_id: lesson.session_id,
            answers: correct_answers(&conn, lesson.session_id),
            writing_response:
                "Guten Tag. Ich heiße Armand und ich komme aus England. Freut mich sehr.".into(),
            speaking_completed: true,
            listened: true,
            confidence: 4,
        },
        "2026-07-21",
    )
    .unwrap();
    assert_eq!(result.level_advanced_to.as_deref(), Some("A2"));
    assert_eq!(result.current_level, "A2");
}

#[test]
fn due_calculation_honors_weekdays_time_and_completion() {
    let monday = chrono::NaiveDate::from_ymd_opt(2026, 7, 20)
        .unwrap()
        .and_hms_opt(8, 0, 0)
        .unwrap();
    assert!(language::slot_due_at(
        7,
        30,
        &[1, 2, 3, 4, 5],
        monday,
        false
    ));
    assert!(!language::slot_due_at(
        9,
        0,
        &[1, 2, 3, 4, 5],
        monday,
        false
    ));
    assert!(!language::slot_due_at(
        7,
        30,
        &[1, 2, 3, 4, 5],
        monday,
        true
    ));
}

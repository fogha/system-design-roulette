//! The study alarm: it stands while an appointment is due, a snooze holds it
//! for a fixed time and never dismisses it, and only two things outrank it.

use principia_desk_lib::{
    alarm,
    classroom::{self, ConfigureClassroomInput, UpsertClassroomSlotInput},
    db,
    domain::schedule::{self, Clock, LocalInstant, ZoneResolver},
    generator::Generator,
    language,
    state::AppState,
};
use rusqlite::Connection;
use std::sync::{atomic::AtomicBool, Arc, Mutex};

/// A fixed zone, so a due appointment is due regardless of the machine.
struct Utc;
impl ZoneResolver for Utc {
    fn resolve(&self, local: chrono::NaiveDateTime) -> LocalInstant {
        LocalInstant::Single(local.and_utc())
    }
    fn name(&self) -> String {
        "UTC".into()
    }
}

fn app_state(dir: &std::path::Path, conn: Connection) -> AppState {
    let mut generator = Generator::new(
        "claude".into(),
        None,
        dir.join("scratch"),
        Arc::new(Mutex::new("opus".into())),
        Arc::new(Mutex::new("claude".into())),
        Arc::new(Mutex::new(String::new())),
        None,
    );
    generator.runner.database = Some(dir.join("principia.db"));
    AppState {
        db: db::Db(Mutex::new(conn)),
        generator,
        data_dir: dir.to_path_buf(),
        locked: AtomicBool::new(false),
        alarm_ringing: AtomicBool::new(false),
        alarm_for: Mutex::new(None),
        panel_height: Mutex::new(600.0),
        debug_day: false,
        escape_failures: Mutex::new(vec![]),
        prev_muted: Mutex::new(None),
        frontend_ready: AtomicBool::new(true),
        class_start_gate: tokio::sync::Mutex::new(()),
        chat_threads: Mutex::new(Default::default()),
        focus: Default::default(),
    }
}

/// A desk with one active class whose study time is already in the past
/// today, so its appointment is due the moment it is materialised.
fn desk_with_due_class() -> (std::path::PathBuf, AppState) {
    let dir = std::env::temp_dir().join(format!(
        "principia-alarm-{}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let conn = db::open(&dir.join("principia.db")).unwrap();
    db::seed_concepts(&conn, include_str!("../seed/concepts.json")).unwrap();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    language::initialize(&conn, &today).unwrap();
    classroom::initialize(&conn).unwrap();
    classroom::upsert_slot(
        &conn,
        &UpsertClassroomSlotInput {
            id: None,
            subject_id: "typescript".into(),
            hour: 0,
            minute: 0,
            weekdays: vec![1, 2, 3, 4, 5, 6, 7],
            enabled: true,
        },
    )
    .unwrap();
    classroom::configure_program(
        &conn,
        &ConfigureClassroomInput {
            subject_id: "typescript".into(),
            enabled: true,
            agent: "claude".into(),
            model: "opus".into(),
            custom_agent_bin: String::new(),
            session_minutes: 30,
            start_level: None,
            target_level: None,
            weekly_minutes: None,
        },
        &today,
    )
    .unwrap();
    // Midnight today has passed, so the appointment is due now.
    let clock = Clock {
        zone: &Utc,
        now: chrono::Utc::now(),
        today: chrono::NaiveDate::parse_from_str(&today, "%Y-%m-%d").unwrap(),
    };
    schedule::materialize(&conn, &clock, false).unwrap();
    (dir.clone(), app_state(&dir, conn))
}

#[test]
fn an_alarm_stands_for_a_due_appointment_until_the_lesson_starts() {
    let (_, state) = desk_with_due_class();
    let alarm = alarm::current(&state).expect("a due appointment rings");
    assert_eq!(alarm.course_id, "typescript");
    assert_eq!(alarm.snoozed_until, None);
    assert_eq!(alarm.queued, 0);

    // Starting the lesson claims the appointment, and the alarm has nothing
    // left to stand for.
    {
        let conn = state.db.0.lock().unwrap();
        schedule::claim(
            &conn,
            &alarm.occurrence_id,
            "typescript",
            "study-1",
            chrono::Utc::now(),
        )
        .unwrap();
    }
    assert_eq!(alarm::current(&state), None);
}

#[test]
fn a_snooze_holds_the_alarm_for_a_fixed_time_and_is_never_a_dismissal() {
    let (_, state) = desk_with_due_class();
    let alarm = alarm::current(&state).unwrap();

    // Only the offered lengths are accepted: a snooze cannot become "never".
    assert!(alarm::snooze(&state, &alarm.occurrence_id, 0).is_err());
    assert!(alarm::snooze(&state, &alarm.occurrence_id, 600).is_err());
    assert!(alarm::snooze(&state, "no-such-appointment", 5).is_err());

    let until = alarm::snooze(&state, &alarm.occurrence_id, 10).unwrap();
    let held = alarm::current(&state).expect("the appointment is still due");
    assert_eq!(held.occurrence_id, alarm.occurrence_id);
    assert_eq!(held.snoozed_until.as_deref(), Some(until.as_str()));
    let ends = chrono::DateTime::parse_from_rfc3339(&until).unwrap();
    let minutes = (ends.with_timezone(&chrono::Utc) - chrono::Utc::now()).num_minutes();
    assert!(
        (9..=10).contains(&minutes),
        "a ten-minute snooze ends in ten minutes, not later"
    );

    // A snooze that has passed no longer holds anything.
    {
        let conn = state.db.0.lock().unwrap();
        db::set_config(
            &conn,
            &format!("alarm_snooze:{}", alarm.occurrence_id),
            &(chrono::Utc::now() - chrono::Duration::minutes(1)).to_rfc3339(),
        )
        .unwrap();
    }
    assert_eq!(alarm::current(&state).unwrap().snoozed_until, None);
}

#[test]
fn pausing_the_schedule_and_the_release_token_outrank_the_alarm() {
    let (dir, state) = desk_with_due_class();
    assert!(alarm::current(&state).is_some());
    {
        let conn = state.db.0.lock().unwrap();
        db::set_config(&conn, "schedule_paused", "1").unwrap();
    }
    assert_eq!(
        alarm::current(&state),
        None,
        "a paused schedule owes nothing, so nothing rings"
    );
    {
        let conn = state.db.0.lock().unwrap();
        db::set_config(&conn, "schedule_paused", "0").unwrap();
    }
    assert!(alarm::current(&state).is_some());
    // The release token is the floor of the safety ladder and silences this too.
    let token = std::env::temp_dir().join(principia_desk_lib::kiosk::UNLOCK_TOKEN);
    std::fs::write(&token, b"").unwrap();
    let silenced = alarm::current(&state);
    let _ = std::fs::remove_file(&token);
    assert_eq!(silenced, None, "a release token silences the alarm");
    let _ = std::fs::remove_dir_all(dir);
}

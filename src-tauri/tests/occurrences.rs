//! Durable appointments: one per rule per local date, due/missed transitions,
//! consumption exactly once, make-up, pause, rule edits and DST resolution.
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use rusqlite::Connection;
use system_design_roulette_lib::{
    classroom::{self, ConfigureClassroomInput, UpsertClassroomSlotInput},
    db,
    domain::schedule::{self, Clock, LocalInstant, ZoneResolver},
    language,
    subjects::engineering,
};

/// A zone two hours ahead of UTC on most days, with a clocks-forward gap
/// 02:00–03:00 on `gap_day` (winter offset +1 before it) and a clocks-back
/// repeat 02:00–03:00 on `fold_day`.
struct ScriptedZone {
    gap_day: NaiveDate,
    fold_day: NaiveDate,
}
impl ScriptedZone {
    fn offset_hours(&self, local: NaiveDateTime) -> i64 {
        let three = NaiveTime::from_hms_opt(3, 0, 0).unwrap();
        let date = local.date();
        let after_gap = date > self.gap_day || (date == self.gap_day && local.time() >= three);
        let before_fold = date < self.fold_day || (date == self.fold_day && local.time() < three);
        if after_gap && before_fold {
            2
        } else {
            1
        }
    }
}
impl ZoneResolver for ScriptedZone {
    fn resolve(&self, local: NaiveDateTime) -> LocalInstant {
        let time = local.time();
        let window = time >= NaiveTime::from_hms_opt(2, 0, 0).unwrap()
            && time < NaiveTime::from_hms_opt(3, 0, 0).unwrap();
        if local.date() == self.gap_day && window {
            return LocalInstant::Nonexistent;
        }
        if local.date() == self.fold_day && window {
            return LocalInstant::Ambiguous(
                Utc.from_utc_datetime(&(local - chrono::Duration::hours(2))),
                Utc.from_utc_datetime(&(local - chrono::Duration::hours(1))),
            );
        }
        LocalInstant::Single(
            Utc.from_utc_datetime(&(local - chrono::Duration::hours(self.offset_hours(local)))),
        )
    }
    fn name(&self) -> String {
        "scripted".into()
    }
}

fn fixture() -> Connection {
    let dir = std::env::temp_dir().join(format!(
        "principia-occurrences-{}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let conn = db::open(&dir.join("learner.db")).unwrap();
    db::seed_concepts(&conn, include_str!("../seed/concepts.json")).unwrap();
    language::initialize(&conn, "2026-09-14").unwrap();
    classroom::initialize(&conn).unwrap();
    conn
}
fn rule(conn: &Connection, subject: &str, hour: u32, minute: u32, weekdays: Vec<u8>) -> i64 {
    classroom::upsert_slot(
        conn,
        &UpsertClassroomSlotInput {
            id: None,
            subject_id: subject.into(),
            hour,
            minute,
            weekdays,
            enabled: true,
        },
    )
    .unwrap()
}
fn activate(conn: &Connection, subject: &str) {
    classroom::configure_program(
        conn,
        &ConfigureClassroomInput {
            subject_id: subject.into(),
            enabled: true,
            agent: "claude".into(),
            model: "sonnet".into(),
            custom_agent_bin: String::new(),
            session_minutes: 30,
            start_level: None,
            target_level: None,
            weekly_minutes: None,
        },
        "2026-09-14",
    )
    .unwrap();
}
fn at<'a>(zone: &'a dyn ZoneResolver, date: &str, time: &str) -> Clock<'a> {
    let today = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
    let local = today.and_time(NaiveTime::parse_from_str(time, "%H:%M").unwrap());
    let now = match zone.resolve(local) {
        LocalInstant::Single(t) | LocalInstant::Ambiguous(t, _) => t,
        LocalInstant::Nonexistent => panic!("test clock in a gap"),
    };
    Clock { zone, now, today }
}
fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}
const ZONE: ScriptedZone = ScriptedZone {
    gap_day: NaiveDate::MIN,
    fold_day: NaiveDate::MIN,
};

#[test]
fn one_appointment_per_rule_per_day_becomes_due_at_its_time_and_is_consumed_once() {
    let conn = fixture();
    // 2026-09-14 is a Monday.
    let id = rule(&conn, "typescript", 9, 0, vec![1, 2, 3, 4, 5]);
    activate(&conn, "typescript");
    let early = schedule::materialize(&conn, &at(&ZONE, "2026-09-14", "08:00"), false).unwrap();
    assert_eq!(early.len(), 1);
    assert_eq!(early[0].disposition, "scheduled");
    assert_eq!(
        (early[0].local_time.as_str(), early[0].local_date.as_str()),
        ("09:00", "2026-09-14")
    );
    assert_eq!(
        early[0].fires_at, "2026-09-14T08:00:00.000Z",
        "09:00 local at +1"
    );
    assert_eq!(
        schedule::materialize(&conn, &at(&ZONE, "2026-09-14", "08:30"), false)
            .unwrap()
            .len(),
        1,
        "no duplicate per rule and day"
    );
    let views = classroom::slot_views(&conn, "2026-09-14", false).unwrap();
    assert!(!views[0].owed);
    assert_eq!(
        views[0].occurrence_id.as_deref(),
        Some(early[0].id.as_str())
    );
    let due = schedule::materialize(&conn, &at(&ZONE, "2026-09-14", "09:01"), false).unwrap();
    assert_eq!(due[0].disposition, "due");
    assert!(classroom::slot_views(&conn, "2026-09-14", false).unwrap()[0].owed);
    let claimed = schedule::claim(
        &conn,
        &due[0].id,
        "typescript",
        "study:one",
        utc("2026-09-14T08:05:00Z"),
    )
    .unwrap();
    assert_eq!(claimed.disposition, "started");
    assert!(schedule::claim(
        &conn,
        &due[0].id,
        "typescript",
        "study:two",
        utc("2026-09-14T08:06:00Z")
    )
    .is_err());
    assert!(schedule::claim(
        &conn,
        &due[0].id,
        "javascript",
        "study:one",
        utc("2026-09-14T08:06:00Z")
    )
    .is_err());
    let view = classroom::slot_views(&conn, "2026-09-14", false)
        .unwrap()
        .remove(0);
    assert!(view.in_progress && !view.owed);
    assert!(classroom::validate_slot_start(&conn, "typescript", id, "2026-09-14").is_err());
    schedule::resolve(&conn, "study:one", true, utc("2026-09-14T08:40:00Z")).unwrap();
    let done = schedule::get(&conn, &due[0].id).unwrap();
    assert_eq!(done.disposition, "completed");
    assert!(
        schedule::skip(&conn, &due[0].id, utc("2026-09-14T09:00:00Z")).is_err(),
        "a finished appointment cannot change"
    );
    // Tomorrow starts fresh.
    let next = schedule::materialize(&conn, &at(&ZONE, "2026-09-15", "07:00"), false).unwrap();
    assert_eq!(next.len(), 1);
    assert_ne!(next[0].id, due[0].id);
}

#[test]
fn unstarted_appointments_become_missed_history_that_can_be_made_up_or_skipped() {
    let conn = fixture();
    rule(&conn, "javascript", 9, 0, vec![1, 2, 3, 4, 5, 6, 7]);
    rule(&conn, "typescript", 11, 0, vec![1, 2, 3, 4, 5, 6, 7]);
    activate(&conn, "javascript");
    activate(&conn, "typescript");
    schedule::materialize(&conn, &at(&ZONE, "2026-09-14", "12:00"), false).unwrap();
    let agenda = schedule::materialize(&conn, &at(&ZONE, "2026-09-15", "08:00"), false).unwrap();
    let missed: Vec<_> = agenda
        .iter()
        .filter(|o| o.disposition == "missed")
        .collect();
    assert_eq!(
        missed.len(),
        2,
        "both unstarted appointments of yesterday are missed"
    );
    assert!(
        agenda
            .iter()
            .filter(|o| o.local_date == "2026-09-15")
            .count()
            == 2
    );
    let js = missed.iter().find(|o| o.course_id == "javascript").unwrap();
    let ts = missed.iter().find(|o| o.course_id == "typescript").unwrap();
    let made_up = schedule::claim(
        &conn,
        &js.id,
        "javascript",
        "study:makeup",
        utc("2026-09-15T07:00:00Z"),
    )
    .unwrap();
    assert_eq!(made_up.disposition, "started");
    schedule::resolve(&conn, "study:makeup", true, utc("2026-09-15T07:30:00Z")).unwrap();
    assert_eq!(
        schedule::get(&conn, &js.id).unwrap().disposition,
        "completed"
    );
    let skipped = schedule::skip(&conn, &ts.id, utc("2026-09-15T07:00:00Z")).unwrap();
    assert_eq!(skipped.disposition, "skipped");
    assert!(schedule::claim(
        &conn,
        &ts.id,
        "typescript",
        "study:late",
        utc("2026-09-15T07:31:00Z")
    )
    .is_err());
    let history = schedule::history(&conn, "typescript", 10).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[1].disposition, "skipped");
    // Missed history ages out of the agenda after the make-up window.
    let later = schedule::materialize(&conn, &at(&ZONE, "2026-10-05", "08:00"), false).unwrap();
    assert!(later.iter().all(|o| o.local_date == "2026-10-05"));
    assert!(later.iter().all(|o| o.disposition != "missed"));
}

#[test]
fn paused_scheduling_creates_no_appointments_and_resuming_does() {
    let conn = fixture();
    rule(&conn, "linux-bash", 7, 30, vec![1, 2, 3, 4, 5, 6, 7]);
    activate(&conn, "linux-bash");
    assert!(
        schedule::materialize(&conn, &at(&ZONE, "2026-09-14", "08:00"), true)
            .unwrap()
            .is_empty()
    );
    let resumed = schedule::materialize(&conn, &at(&ZONE, "2026-09-14", "08:00"), false).unwrap();
    assert_eq!(resumed.len(), 1);
    assert_eq!(resumed[0].disposition, "due");
    db::set_config(&conn, "schedule_paused", "1").unwrap();
    assert!(
        !classroom::slot_views(&conn, "2026-09-14", false).unwrap()[0].owed,
        "paused appointments are not owed"
    );
}

#[test]
fn clock_changes_fire_once_and_move_a_nonexistent_time_forward() {
    let conn = fixture();
    let zone = ScriptedZone {
        gap_day: NaiveDate::from_ymd_opt(2026, 3, 29).unwrap(),
        fold_day: NaiveDate::from_ymd_opt(2026, 10, 25).unwrap(),
    };
    rule(&conn, "german", 2, 30, vec![1, 2, 3, 4, 5, 6, 7]);
    activate(&conn, "german");
    let gap = schedule::materialize(&conn, &at(&zone, "2026-03-29", "01:00"), false).unwrap();
    assert_eq!(gap[0].local_time, "02:30");
    assert_eq!(
        gap[0].fires_at, "2026-03-29T01:00:00.000Z",
        "02:30 does not exist; fires at 03:00 local (+2) = 01:00Z"
    );
    let fold = schedule::materialize(&conn, &at(&zone, "2026-10-25", "01:00"), false).unwrap();
    let repeated = fold.iter().find(|o| o.local_date == "2026-10-25").unwrap();
    assert_eq!(
        repeated.fires_at, "2026-10-25T00:30:00.000Z",
        "the first 02:30 (+2) wins; the repeated hour does not fire twice"
    );
    assert_eq!(
        fold.iter().filter(|o| o.local_date == "2026-10-25").count(),
        1
    );
}

#[test]
fn editing_a_rule_refreshes_unfired_appointments_and_deleting_keeps_history() {
    let conn = fixture();
    let id = rule(&conn, "typescript", 9, 0, vec![1, 2, 3, 4, 5, 6, 7]);
    activate(&conn, "typescript");
    let first = schedule::materialize(&conn, &at(&ZONE, "2026-09-14", "08:00"), false)
        .unwrap()
        .remove(0);
    classroom::upsert_slot(
        &conn,
        &UpsertClassroomSlotInput {
            id: Some(id),
            subject_id: "typescript".into(),
            hour: 10,
            minute: 15,
            weekdays: vec![1, 2, 3, 4, 5, 6, 7],
            enabled: true,
        },
    )
    .unwrap();
    let refreshed = schedule::materialize(&conn, &at(&ZONE, "2026-09-14", "08:05"), false)
        .unwrap()
        .remove(0);
    assert_eq!(
        refreshed.id, first.id,
        "the unfired appointment keeps its identity"
    );
    assert_eq!(
        (refreshed.local_time.as_str(), refreshed.rule_revision),
        ("10:15", 2)
    );
    let claimed = schedule::claim(
        &conn,
        &refreshed.id,
        "typescript",
        "study:kept",
        utc("2026-09-14T08:20:00Z"),
    )
    .unwrap();
    // A consumed appointment keeps its snapshot even if the rule changes again.
    classroom::upsert_slot(
        &conn,
        &UpsertClassroomSlotInput {
            id: Some(id),
            subject_id: "typescript".into(),
            hour: 11,
            minute: 0,
            weekdays: vec![1, 2, 3, 4, 5, 6, 7],
            enabled: true,
        },
    )
    .unwrap();
    let kept = schedule::materialize(&conn, &at(&ZONE, "2026-09-14", "08:30"), false)
        .unwrap()
        .remove(0);
    assert_eq!(
        (
            kept.local_time.as_str(),
            kept.rule_revision,
            kept.disposition.as_str()
        ),
        ("10:15", 2, "started")
    );
    schedule::resolve(&conn, "study:kept", true, utc("2026-09-14T09:00:00Z")).unwrap();
    // Tomorrow's unfired appointment disappears with the rule; history survives detached.
    schedule::materialize(&conn, &at(&ZONE, "2026-09-15", "08:00"), false).unwrap();
    conn.execute("UPDATE study_sessions SET status='completed' WHERE 0", [])
        .unwrap();
    classroom::delete_slot(&conn, id).unwrap();
    let history = schedule::history(&conn, "typescript", 10).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(
        (
            history[0].id.as_str(),
            history[0].rule_id,
            history[0].disposition.as_str()
        ),
        (claimed.id.as_str(), None, "completed")
    );
}

#[test]
fn a_class_lesson_started_from_an_appointment_resolves_it_on_completion() {
    let conn = fixture();
    let id = rule(&conn, "javascript", 9, 0, vec![1, 2, 3, 4, 5, 6, 7]);
    activate(&conn, "javascript");
    let due = schedule::materialize(&conn, &at(&ZONE, "2026-09-14", "09:30"), false)
        .unwrap()
        .remove(0);
    assert_eq!(due.disposition, "due");
    let program = classroom::program_row(&conn, "javascript").unwrap();
    let planned = engineering::plan(
        &conn,
        &program,
        Some(id),
        Some(due.id.clone()),
        "2026-09-14",
        false,
    )
    .unwrap();
    assert_eq!(
        engineering::selection(&planned)
            .unwrap()
            .occurrence_id
            .as_deref(),
        Some(due.id.as_str())
    );
    schedule::claim(
        &conn,
        &due.id,
        "javascript",
        &format!("study:{}", planned.id.0),
        utc("2026-09-14T07:31:00Z"),
    )
    .unwrap();
    assert!(classroom::slot_views(&conn, "2026-09-14", false).unwrap()[0].in_progress);
    let skipped = engineering::skip(&conn, &planned.id).unwrap();
    assert_eq!(
        skipped.status,
        system_design_roulette_lib::domain::sessions::Status::Skipped
    );
    assert_eq!(
        schedule::get(&conn, &due.id).unwrap().disposition,
        "skipped"
    );
    assert!(
        classroom::validate_slot_start(&conn, "javascript", id, "2026-09-14").is_err(),
        "the day's appointment is consumed"
    );
}

#[test]
fn an_unfired_appointment_can_move_within_its_day_and_keeps_the_move_until_the_rule_changes() {
    let conn = fixture();
    let rule_id = rule(&conn, "typescript", 9, 0, vec![1, 2, 3, 4, 5, 6, 7]);
    activate(&conn, "typescript");
    let morning = at(&ZONE, "2026-09-14", "08:00");
    schedule::materialize(&conn, &morning, false).unwrap();
    let occurrence = schedule::today_for_rule(&conn, rule_id, "2026-09-14")
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            occurrence.disposition.as_str(),
            occurrence.local_time.as_str()
        ),
        ("scheduled", "09:00")
    );
    // Moving earlier than now makes it due at once; moving later keeps it scheduled.
    let moved = schedule::reschedule(&conn, &occurrence.id, "07:30", &ZONE, morning.now).unwrap();
    assert_eq!(
        (moved.disposition.as_str(), moved.local_time.as_str()),
        ("due", "07:30")
    );
    assert!(moved.fires_at < occurrence.fires_at);
    let later = schedule::reschedule(&conn, &occurrence.id, "18:15", &ZONE, morning.now).unwrap();
    assert_eq!(
        (later.disposition.as_str(), later.local_time.as_str()),
        ("scheduled", "18:15")
    );
    assert_eq!(
        schedule::unfired_times_today(&conn, "2026-09-14").unwrap(),
        vec![(18, 15)]
    );
    // Materializing again keeps the move while the rule is unchanged.
    schedule::materialize(&conn, &at(&ZONE, "2026-09-14", "08:05"), false).unwrap();
    assert_eq!(
        schedule::get(&conn, &occurrence.id).unwrap().local_time,
        "18:15"
    );
    assert!(
        schedule::reschedule(&conn, &occurrence.id, "7pm", &ZONE, morning.now)
            .unwrap_err()
            .to_string()
            .contains("HH:MM")
    );
    // Skipped appointments cannot move.
    schedule::skip(&conn, &occurrence.id, at(&ZONE, "2026-09-14", "08:10").now).unwrap();
    assert!(schedule::reschedule(&conn, &occurrence.id, "10:00", &ZONE, morning.now).is_err());
}

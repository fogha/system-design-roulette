//! Durable appointments. Recurring study times (rules) produce one occurrence
//! per rule per local service date. An occurrence snapshots its rule revision,
//! intended local time and resolved instant; its disposition moves
//! `scheduled → due → started → completed/skipped`, or to `missed` once its
//! day has passed unstarted. A missed appointment can still be made up.
use crate::db::{DbError, Result};
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};

/// Missed appointments stay visible for make-up this long.
pub const MISSED_HISTORY_DAYS: i64 = 14;

pub enum LocalInstant {
    Single(DateTime<Utc>),
    Ambiguous(DateTime<Utc>, DateTime<Utc>),
    Nonexistent,
}

/// Resolves wall-clock times in the learner's zone. Production uses the system
/// zone; tests script transitions.
pub trait ZoneResolver {
    fn resolve(&self, local: NaiveDateTime) -> LocalInstant;
    fn name(&self) -> String;
}

pub struct SystemZone;
impl ZoneResolver for SystemZone {
    fn resolve(&self, local: NaiveDateTime) -> LocalInstant {
        match chrono::Local.from_local_datetime(&local) {
            chrono::LocalResult::Single(t) => LocalInstant::Single(t.with_timezone(&Utc)),
            chrono::LocalResult::Ambiguous(a, b) => {
                LocalInstant::Ambiguous(a.with_timezone(&Utc), b.with_timezone(&Utc))
            }
            chrono::LocalResult::None => LocalInstant::Nonexistent,
        }
    }
    fn name(&self) -> String {
        chrono::Local::now().offset().to_string()
    }
}

/// A repeated wall-clock time (clocks going back) fires once, at its first
/// instant; a nonexistent time (clocks going forward) moves to the next valid
/// local minute.
pub fn resolve_local(
    zone: &dyn ZoneResolver,
    date: NaiveDate,
    time: NaiveTime,
) -> Result<DateTime<Utc>> {
    let mut candidate = date.and_time(time);
    for _ in 0..=180 {
        match zone.resolve(candidate) {
            LocalInstant::Single(instant) | LocalInstant::Ambiguous(instant, _) => {
                return Ok(instant)
            }
            LocalInstant::Nonexistent => candidate += Duration::minutes(1),
        }
    }
    Err(DbError::Invalid(
        "could not resolve a local study time in this timezone".into(),
    ))
}

/// The learner's clock: the resolver, the current instant and today's local
/// service date, so decisions and countdowns share one source.
pub struct Clock<'a> {
    pub zone: &'a dyn ZoneResolver,
    pub now: DateTime<Utc>,
    pub today: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Occurrence {
    pub id: String,
    pub rule_id: Option<i64>,
    pub course_id: String,
    pub rule_revision: i64,
    pub local_date: String,
    pub local_time: String,
    pub timezone: String,
    pub fires_at: String,
    pub duration_minutes: i64,
    pub disposition: String,
    pub session_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub resolved_at: Option<String>,
}
impl Occurrence {
    pub fn consumed(&self) -> bool {
        matches!(
            self.disposition.as_str(),
            "started" | "completed" | "skipped"
        )
    }
    pub fn open(&self) -> bool {
        matches!(self.disposition.as_str(), "scheduled" | "due" | "missed")
    }
}

const COLUMNS: &str = "id,rule_id,course_id,rule_revision,local_date,local_time,timezone,fires_at,duration_minutes,disposition,session_ref,created_at,updated_at,resolved_at";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Occurrence> {
    Ok(Occurrence {
        id: r.get(0)?,
        rule_id: r.get(1)?,
        course_id: r.get(2)?,
        rule_revision: r.get(3)?,
        local_date: r.get(4)?,
        local_time: r.get(5)?,
        timezone: r.get(6)?,
        fires_at: r.get(7)?,
        duration_minutes: r.get(8)?,
        disposition: r.get(9)?,
        session_ref: r.get(10)?,
        created_at: r.get(11)?,
        updated_at: r.get(12)?,
        resolved_at: r.get(13)?,
    })
}
fn stamp(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn invalid(message: &str) -> DbError {
    DbError::Invalid(message.into())
}

pub fn get(conn: &Connection, id: &str) -> Result<Occurrence> {
    Ok(conn.query_row(
        &format!("SELECT {COLUMNS} FROM schedule_occurrences WHERE id=?1"),
        [id],
        row,
    )?)
}
pub fn today_for_rule(conn: &Connection, rule_id: i64, today: &str) -> Result<Option<Occurrence>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {COLUMNS} FROM schedule_occurrences WHERE rule_id=?1 AND local_date=?2"
            ),
            params![rule_id, today],
            row,
        )
        .optional()?)
}
pub fn for_session(conn: &Connection, session_ref: &str) -> Result<Option<Occurrence>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM schedule_occurrences WHERE session_ref=?1"),
            [session_ref],
            row,
        )
        .optional()?)
}

/// Today's appointments plus missed ones from the recent past, oldest first.
pub fn agenda(conn: &Connection, today: &str) -> Result<Vec<Occurrence>> {
    let floor = NaiveDate::parse_from_str(today, "%Y-%m-%d")
        .map_err(|_| invalid("invalid service date"))?
        - Duration::days(MISSED_HISTORY_DAYS);
    let mut statement = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM schedule_occurrences
         WHERE local_date=?1 OR (disposition='missed' AND local_date>=?2 AND local_date<?1)
         ORDER BY local_date, fires_at, id"
    ))?;
    let rows = statement
        .query_map(params![today, floor.to_string()], row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Appointment history for one class, newest first.
pub fn history(conn: &Connection, course_id: &str, limit: i64) -> Result<Vec<Occurrence>> {
    let mut statement = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM schedule_occurrences WHERE course_id=?1
         ORDER BY local_date DESC, fires_at DESC LIMIT ?2"
    ))?;
    let rows = statement
        .query_map(params![course_id, limit], row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

struct RuleRow {
    id: i64,
    course_id: String,
    hour: u32,
    minute: u32,
    weekdays: Vec<u8>,
    revision: i64,
    session_minutes: i64,
}

fn active_rules(conn: &Connection) -> Result<Vec<RuleRow>> {
    let mut statement = conn.prepare(
        "SELECT s.id, s.subject_id, s.hour, s.minute, s.weekdays_json, s.revision, p.session_minutes
         FROM classroom_schedule_slots s JOIN classroom_programs p ON p.subject_id = s.subject_id
         WHERE s.enabled = 1 AND p.enabled = 1 ORDER BY s.id",
    )?;
    let rows = statement
        .query_map([], |r| {
            Ok(RuleRow {
                id: r.get(0)?,
                course_id: r.get(1)?,
                hour: r.get::<_, i64>(2)? as u32,
                minute: r.get::<_, i64>(3)? as u32,
                weekdays: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default(),
                revision: r.get(5)?,
                session_minutes: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Legacy session rows still consume today's appointment for their rule.
fn legacy_consumption(
    conn: &Connection,
    rule_id: i64,
    today: &str,
) -> Result<Option<(String, String)>> {
    let engineering: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, status FROM classroom_sessions WHERE slot_id=?1 AND session_date=?2 ORDER BY id DESC LIMIT 1",
            params![rule_id, today],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((id, status)) = engineering {
        return Ok(Some((format!("classroom:{id}"), status)));
    }
    let language: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, status FROM language_sessions WHERE classroom_slot_id=?1 AND session_date=?2 ORDER BY id DESC LIMIT 1",
            params![rule_id, today],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(language.map(|(id, status)| (format!("language:{id}"), status)))
}

/// Bring appointments up to date for the learner's clock: past unstarted
/// appointments become missed history, today's appointments exist once per
/// enabled rule of an active class (none while scheduling is paused), unfired
/// ones follow rule edits, and due ones are marked. Returns today's agenda.
pub fn materialize(conn: &Connection, clock: &Clock<'_>, paused: bool) -> Result<Vec<Occurrence>> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let today = clock.today.to_string();
    let now = stamp(clock.now);
    tx.execute(
        "UPDATE schedule_occurrences SET disposition='missed', resolved_at=?2, updated_at=?2
         WHERE disposition IN ('scheduled','due') AND local_date < ?1",
        params![today, now],
    )?;
    if !paused {
        let weekday = clock.today.weekday().number_from_monday() as u8;
        let rules = active_rules(&tx)?;
        for rule in &rules {
            let existing = today_for_rule(&tx, rule.id, &today)?;
            if !rule.weekdays.contains(&weekday) {
                if let Some(existing) =
                    existing.filter(|o| !o.consumed() && o.disposition != "missed")
                {
                    tx.execute(
                        "DELETE FROM schedule_occurrences WHERE id=?1",
                        [&existing.id],
                    )?;
                }
                continue;
            }
            let time = NaiveTime::from_hms_opt(rule.hour, rule.minute, 0)
                .ok_or_else(|| invalid("invalid rule time"))?;
            let fires_at = resolve_local(clock.zone, clock.today, time)?;
            let local_time = format!("{:02}:{:02}", rule.hour, rule.minute);
            match existing {
                None => {
                    let id = format!("occurrence-{:032x}", rand::random::<u128>());
                    tx.execute(
                        "INSERT INTO schedule_occurrences (id,rule_id,course_id,rule_revision,local_date,local_time,timezone,fires_at,duration_minutes,disposition,created_at,updated_at)
                         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'scheduled',?10,?10)",
                        params![id, rule.id, rule.course_id, rule.revision, today, local_time, clock.zone.name(), stamp(fires_at), rule.session_minutes, now],
                    )?;
                    if let Some((session_ref, status)) = legacy_consumption(&tx, rule.id, &today)? {
                        let disposition = match status.as_str() {
                            "in_progress" => "started",
                            "skipped" => "skipped",
                            _ => "completed",
                        };
                        tx.execute(
                            "UPDATE schedule_occurrences SET disposition=?2, session_ref=?3, resolved_at=?4, updated_at=?4 WHERE id=?1",
                            params![id, disposition, session_ref, now],
                        )?;
                    }
                }
                Some(existing)
                    if !existing.consumed() && existing.rule_revision != rule.revision =>
                {
                    // Unfired appointments follow the edited rule; consumed ones keep their snapshot.
                    tx.execute(
                        "UPDATE schedule_occurrences SET rule_revision=?2, local_time=?3, fires_at=?4, duration_minutes=?5, timezone=?6, disposition=CASE WHEN disposition='due' THEN 'scheduled' ELSE disposition END, updated_at=?7 WHERE id=?1",
                        params![existing.id, rule.revision, local_time, stamp(fires_at), rule.session_minutes, clock.zone.name(), now],
                    )?;
                }
                Some(_) => {}
            }
        }
        tx.execute(
            "UPDATE schedule_occurrences SET disposition='due', updated_at=?2
             WHERE disposition='scheduled' AND local_date=?1 AND fires_at<=?2",
            params![today, now],
        )?;
    }
    let result = agenda(&tx, &today)?;
    tx.commit()?;
    Ok(result)
}

/// Start (or make up) an appointment with a study session. Consumes it once.
pub fn claim(
    conn: &Connection,
    occurrence_id: &str,
    course_id: &str,
    session_ref: &str,
    now: DateTime<Utc>,
) -> Result<Occurrence> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let occurrence = get(&tx, occurrence_id)?;
    if occurrence.course_id != course_id {
        return Err(invalid("This appointment belongs to another class."));
    }
    if occurrence.session_ref.as_deref() == Some(session_ref) {
        return Ok(occurrence);
    }
    if occurrence.consumed() {
        return Err(invalid("This appointment was already started or resolved."));
    }
    tx.execute(
        "UPDATE schedule_occurrences SET disposition='started', session_ref=?2, resolved_at=?3, updated_at=?3 WHERE id=?1",
        params![occurrence_id, session_ref, stamp(now)],
    )?;
    let updated = get(&tx, occurrence_id)?;
    tx.commit()?;
    Ok(updated)
}

/// Record the outcome of the session that started an appointment. Idempotent;
/// a session without an appointment resolves nothing.
pub fn resolve(
    conn: &Connection,
    session_ref: &str,
    completed: bool,
    now: DateTime<Utc>,
) -> Result<()> {
    let disposition = if completed { "completed" } else { "skipped" };
    conn.execute(
        "UPDATE schedule_occurrences SET disposition=?2, resolved_at=?3, updated_at=?3
         WHERE session_ref=?1 AND disposition='started'",
        params![session_ref, disposition, stamp(now)],
    )?;
    Ok(())
}

/// Explicitly skip an open appointment without starting a session.
pub fn skip(conn: &Connection, occurrence_id: &str, now: DateTime<Utc>) -> Result<Occurrence> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let occurrence = get(&tx, occurrence_id)?;
    if occurrence.disposition == "skipped" {
        return Ok(occurrence);
    }
    if !occurrence.open() {
        return Err(invalid("This appointment already has a session."));
    }
    tx.execute(
        "UPDATE schedule_occurrences SET disposition='skipped', resolved_at=?2, updated_at=?2 WHERE id=?1",
        params![occurrence_id, stamp(now)],
    )?;
    let updated = get(&tx, occurrence_id)?;
    tx.commit()?;
    Ok(updated)
}

/// Unfired appointments of a rule being deleted disappear; consumed and missed
/// ones remain as history without the rule link.
pub fn detach_rule(conn: &Connection, rule_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM schedule_occurrences WHERE rule_id=?1 AND disposition IN ('scheduled','due')",
        [rule_id],
    )?;
    Ok(())
}

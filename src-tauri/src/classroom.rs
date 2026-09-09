use crate::generator::{Exercise, GeneratedCourse, GenerationProfile, Resource};
use crate::{db, language, mastery, roulette, state::AppState};
use chrono::{Datelike, Duration, Local, NaiveDateTime, Timelike, Weekday};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

type Result<T> = std::result::Result<T, String>;

pub use crate::catalog::{CourseDefinition as SubjectSpec, SubjectKind, COURSES as SUBJECTS};

pub fn subject(subject_id: &str) -> Result<&'static SubjectSpec> {
    SUBJECTS
        .iter()
        .find(|candidate| candidate.id == subject_id)
        .ok_or_else(|| format!("unknown classroom subject: {subject_id}"))
}

pub fn initialize(conn: &Connection) -> Result<()> {
    crate::catalog::validate()?;
    let global_agent = db::get_config(conn, "agent")
        .map_err(|error| error.to_string())?
        .unwrap_or_else(|| "claude".into());
    let global_model = db::get_config(conn, "model")
        .map_err(|error| error.to_string())?
        .unwrap_or_else(|| "opus".into());
    let global_custom = db::get_config(conn, "custom_agent_bin")
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    for spec in SUBJECTS {
        let (enabled, minutes) = if spec.kind == SubjectKind::Language {
            conn.query_row(
                "SELECT enabled, session_minutes FROM language_programs WHERE language = ?1",
                [spec.id],
                |row| Ok((row.get::<_, i64>(0)? != 0, row.get::<_, i64>(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .unwrap_or((false, 30))
        } else {
            (false, 30)
        };
        conn.execute(
            "INSERT OR IGNORE INTO classroom_programs
                (subject_id, kind, label, native_label, short_code, enabled, agent,
                 model, custom_agent_bin, prompt_profile, prompt_version,
                 session_minutes, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'v1', ?11, ?12)",
            params![
                spec.id,
                spec.kind.as_str(),
                spec.label,
                spec.native_label,
                spec.short_code,
                i64::from(enabled),
                global_agent,
                global_model,
                global_custom,
                spec.prompt_profile,
                minutes,
                language::now_iso(),
            ],
        )
        .map_err(|error| error.to_string())?;
        // Refresh catalog metadata while retaining enrollment preferences.
        conn.execute(
            "UPDATE classroom_programs SET label = ?2, native_label = ?3,
            short_code = ?4, prompt_profile = ?5, prompt_version = ?6 WHERE subject_id = ?1",
            params![
                spec.id,
                spec.label,
                spec.native_label,
                spec.short_code,
                spec.prompt_profile,
                spec.version
            ],
        )
        .map_err(|error| error.to_string())?;
    }
    migrate_language_slots(conn)?;
    Ok(())
}

fn migrate_language_slots(conn: &Connection) -> Result<()> {
    let key = "migration:classroom_language_slots:v1";
    if db::get_config(conn, key)
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Ok(());
    }
    let transaction = conn
        .unchecked_transaction()
        .map_err(|error| error.to_string())?;
    let conn = &*transaction;
    conn.execute(
        "INSERT OR IGNORE INTO classroom_schedule_slots
            (subject_id, hour, minute, weekdays_json, enabled, created_at)
         SELECT language, hour, minute, weekdays_json, enabled, created_at
         FROM language_schedule_slots",
        [],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "UPDATE language_sessions
         SET classroom_slot_id = (
             SELECT c.id
             FROM language_schedule_slots old
             JOIN classroom_schedule_slots c
               ON c.subject_id = old.language
              AND c.hour = old.hour
              AND c.minute = old.minute
             WHERE old.id = language_sessions.slot_id
         )
         WHERE classroom_slot_id IS NULL AND slot_id IS NOT NULL",
        [],
    )
    .map_err(|error| error.to_string())?;
    db::set_config(conn, key, "1").map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ProgramRow {
    pub subject_id: String,
    pub kind: String,
    pub label: String,
    pub native_label: String,
    pub short_code: String,
    pub enabled: bool,
    pub agent: String,
    pub model: String,
    pub custom_agent_bin: String,
    pub prompt_profile: String,
    pub prompt_version: String,
    pub session_minutes: i64,
    pub learning_goal: String,
    pub target_weekly_minutes: i64,
}

pub fn program_row(conn: &Connection, subject_id: &str) -> Result<ProgramRow> {
    conn.query_row(
        "SELECT subject_id, kind, label, native_label, short_code, enabled,
                agent, model, custom_agent_bin, prompt_profile, prompt_version,
                session_minutes, learning_goal, target_weekly_minutes
         FROM classroom_programs WHERE subject_id = ?1",
        [subject_id],
        |row| {
            Ok(ProgramRow {
                subject_id: row.get(0)?,
                kind: row.get(1)?,
                label: row.get(2)?,
                native_label: row.get(3)?,
                short_code: row.get(4)?,
                enabled: row.get::<_, i64>(5)? != 0,
                agent: row.get(6)?,
                model: row.get(7)?,
                custom_agent_bin: row.get(8)?,
                prompt_profile: row.get(9)?,
                prompt_version: row.get(10)?,
                session_minutes: row.get(11)?,
                learning_goal: row.get(12)?,
                target_weekly_minutes: row.get(13)?,
            })
        },
    )
    .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct ClassroomProgramView {
    pub subject_id: String,
    pub kind: String,
    pub label: String,
    pub native_label: String,
    pub short_code: String,
    pub enabled: bool,
    pub agent: String,
    pub model: String,
    pub custom_agent_bin: String,
    pub prompt_profile: String,
    pub prompt_version: String,
    pub session_minutes: i64,
    pub learning_goal: String,
    pub target_weekly_minutes: i64,
    pub progress: f64,
    pub progress_label: String,
    /// Every module in the track (or the CEFR goal level) is finished.
    /// Completed subjects never start a fresh lesson automatically; only an
    /// explicit revisit is served.
    pub completed: bool,
    pub language_progress: Option<language::LanguageProgramView>,
}

pub fn program_views(conn: &Connection, today: &str) -> Result<Vec<ClassroomProgramView>> {
    SUBJECTS
        .iter()
        .map(|spec| program_view(conn, spec.id, today))
        .collect()
}

pub fn program_view(
    conn: &Connection,
    subject_id: &str,
    today: &str,
) -> Result<ClassroomProgramView> {
    let row = program_row(conn, subject_id)?;
    let language_progress = if row.kind == "language" {
        Some(language::program_view(conn, subject_id, today)?)
    } else {
        None
    };
    let (progress, progress_label, completed) = if let Some(language) = &language_progress {
        let level_done =
            crate::language::level_complete(conn, subject_id, &language.current_level)?;
        let goal_met =
            language.current_level == language.target_level || language.current_level == "B2";
        (
            language.progress,
            format!(
                "{} / {} evidence steps in {}",
                language.completed_steps, language.required_steps, language.current_level
            ),
            level_done && goal_met,
        )
    } else {
        let (total, covered, done): (i64, i64, i64) = conn
            .query_row(
                "SELECT COUNT(*),
                        SUM(CASE WHEN COALESCE(m.state, 'unseen') != 'unseen' THEN 1 ELSE 0 END),
                        SUM(CASE WHEN m.state IN ('mastered','maintenance') THEN 1 ELSE 0 END)
                 FROM concepts c
                 LEFT JOIN mastery m ON m.concept_id = c.id
                 WHERE c.active = 1 AND c.focus = ?1",
                [subject_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|error| error.to_string())?;
        (
            if total == 0 {
                0.0
            } else {
                covered as f64 / total as f64
            },
            format!("{covered} / {total} concepts practiced"),
            total > 0 && done == total,
        )
    };
    Ok(ClassroomProgramView {
        subject_id: row.subject_id,
        kind: row.kind,
        label: row.label,
        native_label: row.native_label,
        short_code: row.short_code,
        enabled: row.enabled,
        agent: row.agent,
        model: row.model,
        custom_agent_bin: row.custom_agent_bin,
        prompt_profile: row.prompt_profile,
        prompt_version: row.prompt_version,
        session_minutes: row.session_minutes,
        learning_goal: row.learning_goal,
        target_weekly_minutes: row.target_weekly_minutes,
        progress,
        progress_label,
        completed,
        language_progress,
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigureClassroomInput {
    pub subject_id: String,
    pub enabled: bool,
    pub agent: String,
    pub model: String,
    #[serde(default)]
    pub custom_agent_bin: String,
    pub session_minutes: i64,
    #[serde(default)]
    pub start_level: Option<String>,
    #[serde(default)]
    pub target_level: Option<String>,
    #[serde(default)]
    pub weekly_minutes: Option<i64>,
}

fn valid_agent(agent: &str) -> bool {
    matches!(
        agent,
        "claude" | "codex" | "cursor" | "gemini" | "deepseek" | "custom"
    )
}

fn valid_model(model: &str) -> bool {
    matches!(model, "opus" | "sonnet" | "haiku")
}

pub fn configure_program(
    conn: &Connection,
    input: &ConfigureClassroomInput,
    today: &str,
) -> Result<()> {
    let spec = subject(&input.subject_id)?;
    if !valid_agent(&input.agent) || !valid_model(&input.model) {
        return Err("class agent or model is invalid".into());
    }
    if input.agent == "custom" {
        if input.custom_agent_bin.trim().is_empty() {
            return Err("custom class agent needs a binary command".into());
        }
        let binary = input
            .custom_agent_bin
            .split_whitespace()
            .next()
            .unwrap_or_default();
        if !std::path::Path::new(binary).exists() {
            return Err(format!("custom class agent binary not found: {binary}"));
        }
    }
    if !(15..=90).contains(&input.session_minutes) {
        return Err("class session length must be between 15 and 90 minutes".into());
    }
    if !input.enabled && has_active_session(conn, &input.subject_id)? {
        return Err("finish the active class session before disabling it".into());
    }
    if spec.kind == SubjectKind::Engineering
        && (input.start_level.is_some()
            || input.target_level.is_some()
            || input.weekly_minutes.is_some())
    {
        return Err("language-only settings cannot be applied to an engineering subject".into());
    }
    conn.execute(
        "UPDATE classroom_programs
         SET enabled = ?2, agent = ?3, model = ?4, custom_agent_bin = ?5,
             session_minutes = ?6, updated_at = ?7
         WHERE subject_id = ?1",
        params![
            input.subject_id,
            i64::from(input.enabled),
            input.agent,
            input.model,
            input.custom_agent_bin.trim(),
            input.session_minutes,
            language::now_iso(),
        ],
    )
    .map_err(|error| error.to_string())?;
    if spec.kind == SubjectKind::Language {
        let existing = language::program_view(conn, spec.id, today)?;
        language::configure_program(
            conn,
            &language::ConfigureProgramInput {
                language: spec.id.into(),
                enabled: input.enabled,
                start_level: input.start_level.clone().unwrap_or(existing.start_level),
                target_level: input.target_level.clone().unwrap_or(existing.target_level),
                weekly_minutes: input.weekly_minutes.unwrap_or(existing.weekly_minutes),
                session_minutes: input.session_minutes,
            },
            today,
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpsertClassroomSlotInput {
    pub id: Option<i64>,
    pub subject_id: String,
    pub hour: u32,
    pub minute: u32,
    pub weekdays: Vec<u8>,
    pub enabled: bool,
}

pub fn upsert_slot(conn: &Connection, input: &UpsertClassroomSlotInput) -> Result<i64> {
    subject(&input.subject_id)?;
    if input.hour > 23 || input.minute > 59 {
        return Err("classroom slot time is invalid".into());
    }
    let mut weekdays = input.weekdays.clone();
    weekdays.sort_unstable();
    weekdays.dedup();
    if weekdays.is_empty() || weekdays.iter().any(|day| !(1..=7).contains(day)) {
        return Err("select at least one valid weekday".into());
    }
    let weekdays_json = serde_json::to_string(&weekdays).map_err(|error| error.to_string())?;
    if let Some(id) = input.id {
        // Hand-editing a slot "claims" it as manual, even if the schedule
        // planner originally created it — re-planning never touches it again.
        let changed = conn
            .execute(
                "UPDATE classroom_schedule_slots
                 SET subject_id = ?2, hour = ?3, minute = ?4,
                     weekdays_json = ?5, enabled = ?6, source = 'manual'
                 WHERE id = ?1",
                params![
                    id,
                    input.subject_id,
                    input.hour,
                    input.minute,
                    weekdays_json,
                    i64::from(input.enabled),
                ],
            )
            .map_err(|error| {
                if error.to_string().contains("UNIQUE constraint failed") {
                    "this class already has a slot at that time".to_string()
                } else {
                    error.to_string()
                }
            })?;
        if changed == 0 {
            return Err("classroom slot was not found".into());
        }
        Ok(id)
    } else {
        conn.execute(
            "INSERT INTO classroom_schedule_slots
                (subject_id, hour, minute, weekdays_json, enabled, source, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'manual', ?6)",
            params![
                input.subject_id,
                input.hour,
                input.minute,
                weekdays_json,
                i64::from(input.enabled),
                language::now_iso(),
            ],
        )
        .map_err(|error| {
            if error.to_string().contains("UNIQUE constraint failed") {
                "this class already has a slot at that time".to_string()
            } else {
                error.to_string()
            }
        })?;
        Ok(conn.last_insert_rowid())
    }
}

pub fn delete_slot(conn: &Connection, id: i64) -> Result<()> {
    let active_engineering: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM classroom_sessions
             WHERE slot_id = ?1 AND status = 'in_progress'",
            [id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let active_language: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM language_sessions
             WHERE classroom_slot_id = ?1 AND status = 'in_progress'",
            [id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if active_engineering + active_language > 0 {
        return Err("finish the active class before deleting this slot".into());
    }
    // Completed sessions keep their history; only the schedule link is cut.
    conn.execute(
        "UPDATE classroom_sessions SET slot_id = NULL
         WHERE slot_id = ?1 AND status != 'in_progress'",
        [id],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "UPDATE language_sessions SET classroom_slot_id = NULL
         WHERE classroom_slot_id = ?1 AND status != 'in_progress'",
        [id],
    )
    .map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM classroom_schedule_slots WHERE id = ?1", [id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// One window of real-world availability the learner supplied to the
/// schedule planner (e.g. "Mon/Wed/Fri 7:00-8:00").
#[derive(Debug, Clone, Deserialize)]
pub struct AvailabilityWindowInput {
    pub weekdays: Vec<u8>,
    pub start_hour: u32,
    pub start_minute: u32,
    pub end_hour: u32,
    pub end_minute: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlanClassroomScheduleInput {
    pub subject_id: String,
    #[serde(default)]
    pub learning_goal: String,
    pub target_weekly_minutes: i64,
    pub windows: Vec<AvailabilityWindowInput>,
    /// false = preview only (no writes); true = persist the plan.
    pub commit: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlannedSlotView {
    pub hour: u32,
    pub minute: u32,
    pub weekdays: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClassroomPlanView {
    pub slots: Vec<PlannedSlotView>,
    pub total_weekly_minutes: i64,
    pub target_weekly_minutes: i64,
    pub meets_target: bool,
    pub program: Option<ClassroomProgramView>,
    pub schedule: Option<Vec<ClassroomSlotView>>,
}

/// One proposed recurring slot per stated availability window, at that
/// window's start time. Windows that resolve to the exact same time merge
/// their weekdays into a single slot instead of colliding.
fn plan_windows(windows: &[AvailabilityWindowInput]) -> Result<Vec<PlannedSlotView>> {
    if windows.is_empty() {
        return Err("add at least one availability window".into());
    }
    let mut slots: Vec<PlannedSlotView> = Vec::new();
    for window in windows {
        if window.start_hour > 23
            || window.end_hour > 23
            || window.start_minute > 59
            || window.end_minute > 59
        {
            return Err("availability window time is invalid".into());
        }
        let mut weekdays = window.weekdays.clone();
        weekdays.sort_unstable();
        weekdays.dedup();
        if weekdays.is_empty() || weekdays.iter().any(|day| !(1..=7).contains(day)) {
            return Err("every availability window needs at least one valid weekday".into());
        }
        if (window.start_hour, window.start_minute) >= (window.end_hour, window.end_minute) {
            return Err("availability window end time must be after its start time".into());
        }
        if let Some(existing) = slots
            .iter_mut()
            .find(|slot| slot.hour == window.start_hour && slot.minute == window.start_minute)
        {
            existing.weekdays.extend(weekdays);
            existing.weekdays.sort_unstable();
            existing.weekdays.dedup();
        } else {
            slots.push(PlannedSlotView {
                hour: window.start_hour,
                minute: window.start_minute,
                weekdays,
            });
        }
    }
    slots.sort_unstable_by_key(|slot| (slot.hour, slot.minute));
    Ok(slots)
}

/// Preview or persist a schedule generated from a learner's stated goal,
/// weekly-minutes target, and availability. Additive to the manual slot
/// editor: committing only replaces this subject's previously *planned*
/// slots, never slots a learner hand-placed with `upsert_slot`.
pub fn plan_schedule(
    conn: &Connection,
    input: &PlanClassroomScheduleInput,
    today: &str,
) -> Result<ClassroomPlanView> {
    let spec = subject(&input.subject_id)?;
    if input.target_weekly_minutes < 0 {
        return Err("target weekly minutes cannot be negative".into());
    }
    let slots = plan_windows(&input.windows)?;
    // Preview and commit share the same collision check. A manual slot owns
    // its time even when its weekdays differ from the proposed recurrence.
    let transaction = conn
        .unchecked_transaction()
        .map_err(|error| error.to_string())?;
    let conn = &*transaction;
    for slot in &slots {
        let manual_collision: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM classroom_schedule_slots
             WHERE subject_id = ?1 AND hour = ?2 AND minute = ?3 AND source = 'manual')",
                params![input.subject_id, slot.hour, slot.minute],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if manual_collision {
            return Err(format!("A manual class already uses {:02}:{:02}. Choose another planning window or edit that class time first.", slot.hour, slot.minute));
        }
    }
    let program = program_row(conn, &input.subject_id)?;
    let total_weekly_minutes: i64 = slots
        .iter()
        .map(|slot| slot.weekdays.len() as i64 * program.session_minutes)
        .sum();
    let meets_target =
        input.target_weekly_minutes == 0 || total_weekly_minutes >= input.target_weekly_minutes;

    let (view_program, view_schedule) = if input.commit {
        conn.execute(
            "UPDATE classroom_programs
             SET learning_goal = ?2, target_weekly_minutes = ?3, updated_at = ?4
             WHERE subject_id = ?1",
            params![
                input.subject_id,
                input.learning_goal.trim(),
                input.target_weekly_minutes,
                language::now_iso(),
            ],
        )
        .map_err(|error| error.to_string())?;
        if spec.kind == SubjectKind::Language {
            let existing = language::program_view(conn, spec.id, today)?;
            language::configure_program(
                conn,
                &language::ConfigureProgramInput {
                    language: spec.id.into(),
                    enabled: true,
                    start_level: existing.start_level,
                    target_level: existing.target_level,
                    weekly_minutes: input.target_weekly_minutes,
                    session_minutes: program.session_minutes,
                },
                today,
            )?;
        }
        conn.execute(
            "DELETE FROM classroom_schedule_slots WHERE subject_id = ?1 AND source = 'planned'",
            [&input.subject_id],
        )
        .map_err(|error| error.to_string())?;
        for slot in &slots {
            let weekdays_json =
                serde_json::to_string(&slot.weekdays).map_err(|error| error.to_string())?;
            conn.execute(
                "INSERT INTO classroom_schedule_slots
                    (subject_id, hour, minute, weekdays_json, enabled, source, created_at)
                 VALUES (?1, ?2, ?3, ?4, 1, 'planned', ?5)
                 ON CONFLICT(subject_id, hour, minute) DO NOTHING",
                params![
                    input.subject_id,
                    slot.hour,
                    slot.minute,
                    weekdays_json,
                    language::now_iso(),
                ],
            )
            .map_err(|error| error.to_string())?;
        }
        let schedule = slot_views(conn, today, false)?
            .into_iter()
            .filter(|view| view.subject_id == input.subject_id)
            .collect();
        (
            Some(program_view(conn, &input.subject_id, today)?),
            Some(schedule),
        )
    } else {
        (None, None)
    };

    transaction.commit().map_err(|error| error.to_string())?;
    Ok(ClassroomPlanView {
        slots,
        total_weekly_minutes,
        target_weekly_minutes: input.target_weekly_minutes,
        meets_target,
        program: view_program,
        schedule: view_schedule,
    })
}

#[derive(Debug, Clone)]
struct SlotRow {
    id: i64,
    subject_id: String,
    label: String,
    short_code: String,
    kind: String,
    hour: u32,
    minute: u32,
    weekdays: Vec<u8>,
    enabled: bool,
    program_enabled: bool,
    source: String,
}

fn slot_rows(conn: &Connection) -> Result<Vec<SlotRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.subject_id, p.label, p.short_code, p.kind,
                    s.hour, s.minute, s.weekdays_json, s.enabled, p.enabled, s.source
             FROM classroom_schedule_slots s
             JOIN classroom_programs p ON p.subject_id = s.subject_id
             ORDER BY s.hour, s.minute, s.id",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let weekdays_json: String = row.get(7)?;
            Ok(SlotRow {
                id: row.get(0)?,
                subject_id: row.get(1)?,
                label: row.get(2)?,
                short_code: row.get(3)?,
                kind: row.get(4)?,
                hour: row.get::<_, i64>(5)? as u32,
                minute: row.get::<_, i64>(6)? as u32,
                weekdays: serde_json::from_str(&weekdays_json).unwrap_or_default(),
                enabled: row.get::<_, i64>(8)? != 0,
                program_enabled: row.get::<_, i64>(9)? != 0,
                source: row.get(10)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn weekday_number(weekday: Weekday) -> u8 {
    weekday.number_from_monday() as u8
}

pub fn slot_due_at(
    hour: u32,
    minute: u32,
    weekdays: &[u8],
    now: NaiveDateTime,
    consumed_today: bool,
) -> bool {
    weekdays.contains(&weekday_number(now.weekday()))
        && !consumed_today
        && (now.hour(), now.minute()) >= (hour, minute)
}

fn slot_state(conn: &Connection, slot: &SlotRow, today: &str) -> Result<(bool, bool)> {
    let status = if slot.kind == "language" {
        conn.query_row(
            "SELECT status FROM language_sessions WHERE classroom_slot_id = ?1
             AND session_date = ?2 ORDER BY id DESC LIMIT 1",
            params![slot.id, today],
            |row| row.get::<_, String>(0),
        )
        .optional()
    } else {
        conn.query_row(
            "SELECT status FROM classroom_sessions WHERE slot_id = ?1
             AND session_date = ?2 ORDER BY id DESC LIMIT 1",
            params![slot.id, today],
            |row| row.get::<_, String>(0),
        )
        .optional()
    };
    status
        .map(|status| {
            let in_progress = status.as_deref() == Some("in_progress");
            (status.is_some(), in_progress)
        })
        .map_err(|error| error.to_string())
}

fn next_fire_at(slot: &SlotRow, now: NaiveDateTime, consumed_today: bool) -> String {
    for offset in 0..=8 {
        let date = now.date() + Duration::days(offset);
        if !slot.weekdays.contains(&weekday_number(date.weekday())) {
            continue;
        }
        let Some(candidate) = date.and_hms_opt(slot.hour, slot.minute, 0) else {
            continue;
        };
        if (offset == 0 && consumed_today) || candidate <= now {
            continue;
        }
        return candidate.format("%Y-%m-%dT%H:%M:%S").to_string();
    }
    now.format("%Y-%m-%dT%H:%M:%S").to_string()
}

#[derive(Debug, Clone, Serialize)]
pub struct CurriculumConceptView {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub category: String,
    pub tier: i64,
    pub phase: String,
    pub core: bool,
    pub prerequisites: Vec<String>,
    pub mastery_state: String,
    pub times_picked: i64,
    pub last_picked_date: Option<String>,
    pub learner_outcome: String,
    pub artifact: String,
    pub related_concepts: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CurriculumMapView {
    pub focus: String,
    pub label: String,
    pub month_outcome: String,
    pub completed_sessions: i64,
    pub current_phase: String,
    pub concepts: Vec<CurriculumConceptView>,
}

pub fn curriculum_map(conn: &Connection, focus: &str) -> Result<CurriculumMapView> {
    crate::focus::validate_selectable(focus).map_err(|error| error.to_string())?;
    let mastery_by_id = mastery::overview(conn, focus)
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|entry| (entry.concept_id, entry.state))
        .collect::<std::collections::HashMap<_, _>>();
    let mut concepts = Vec::new();
    for concept in crate::db::all_concepts(conn, focus).map_err(|error| error.to_string())? {
        let prerequisites_json: String = conn
            .query_row(
                "SELECT prereqs_json FROM concepts WHERE id = ?1",
                [concept.id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        concepts.push(CurriculumConceptView {
            id: concept.id,
            slug: concept.slug,
            title: concept.title,
            category: concept.category,
            tier: concept.tier,
            phase: concept.curriculum.phase,
            core: concept.curriculum.core,
            prerequisites: serde_json::from_str(&prerequisites_json).unwrap_or_default(),
            mastery_state: mastery_by_id
                .get(&concept.id)
                .cloned()
                .unwrap_or_else(|| "unseen".into()),
            times_picked: concept.times_picked,
            last_picked_date: concept.last_picked_date,
            learner_outcome: concept.curriculum.learner_outcome,
            artifact: concept.curriculum.artifact,
            related_concepts: concept.curriculum.related_concepts,
        });
    }
    let phase_order = [
        "foundations",
        "mechanisms",
        "production",
        "synthesis",
        "elective",
    ];
    let current_phase = phase_order
        .into_iter()
        .find(|phase| {
            concepts.iter().any(|concept| {
                concept.core
                    && concept.phase == *phase
                    && !matches!(concept.mastery_state.as_str(), "mastered" | "maintenance")
            })
        })
        .unwrap_or("elective")
        .to_string();
    let completed_sessions: i64 = conn
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM sessions WHERE status = 'completed' AND focus = ?1)
              + (SELECT COUNT(*) FROM classroom_sessions
                 WHERE status = 'completed' AND subject_id = ?1)",
            [focus],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    Ok(CurriculumMapView {
        focus: focus.into(),
        label: crate::focus::label(focus).into(),
        month_outcome: crate::focus::month_outcome(focus).into(),
        completed_sessions,
        current_phase,
        concepts,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ClassroomSlotView {
    pub id: i64,
    pub subject_id: String,
    pub label: String,
    pub short_code: String,
    pub kind: String,
    pub hour: u32,
    pub minute: u32,
    pub weekdays: Vec<u8>,
    pub enabled: bool,
    pub owed: bool,
    pub next_fire_at: String,
    pub in_progress: bool,
    pub source: String,
}

pub fn slot_views(
    conn: &Connection,
    today: &str,
    debug_day: bool,
) -> Result<Vec<ClassroomSlotView>> {
    let paused = matches!(db::get_config(conn, "schedule_paused"), Ok(Some(value)) if value == "1");
    let now = Local::now().naive_local();
    slot_rows(conn)?
        .into_iter()
        .map(|slot| {
            let (consumed, in_progress) = slot_state(conn, &slot, today)?;
            let available = slot.enabled && slot.program_enabled && !paused;
            let owed = available
                && !consumed
                && (debug_day || slot_due_at(slot.hour, slot.minute, &slot.weekdays, now, false));
            let next_fire = next_fire_at(&slot, now, consumed);
            Ok(ClassroomSlotView {
                id: slot.id,
                subject_id: slot.subject_id,
                label: slot.label,
                short_code: slot.short_code,
                kind: slot.kind,
                hour: slot.hour,
                minute: slot.minute,
                weekdays: slot.weekdays,
                enabled: slot.enabled,
                owed,
                next_fire_at: next_fire,
                in_progress,
                source: slot.source,
            })
        })
        .collect()
}

pub fn all_schedule_times(conn: &Connection) -> Result<Vec<(u32, u32)>> {
    let primary_hour = db::get_config(conn, "schedule_hour")
        .map_err(|error| error.to_string())?
        .and_then(|value| value.parse().ok())
        .unwrap_or(9);
    let primary_minute = db::get_config(conn, "schedule_minute")
        .map_err(|error| error.to_string())?
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let mut times = vec![(primary_hour, primary_minute)];
    times.extend(
        slot_rows(conn)?
            .into_iter()
            .filter(|slot| slot.enabled && slot.program_enabled)
            .map(|slot| (slot.hour, slot.minute)),
    );
    times.sort_unstable();
    times.dedup();
    Ok(times)
}

fn has_active_session(conn: &Connection, subject_id: &str) -> Result<bool> {
    let engineering: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM classroom_sessions
             WHERE subject_id = ?1 AND status = 'in_progress'",
            [subject_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let language: i64 = if matches!(subject(subject_id)?.kind, SubjectKind::Language) {
        conn.query_row(
            "SELECT COUNT(*) FROM language_sessions
             WHERE language = ?1 AND status = 'in_progress'",
            [subject_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?
    } else {
        0
    };
    Ok(engineering + language > 0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredQuestion {
    id: usize,
    prompt: String,
    choices: Vec<String>,
    correct_index: usize,
    explanation: String,
    #[serde(default)]
    section: String,
    learning_objective: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredEngineeringLesson {
    concept_id: i64,
    concept_title: String,
    category: String,
    title: String,
    markdown: String,
    resources: Vec<Resource>,
    questions: Vec<StoredQuestion>,
    exercise: Option<Exercise>,
    source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClassroomQuestionView {
    pub id: usize,
    pub prompt: String,
    pub choices: Vec<String>,
    pub section: String,
    pub learning_objective: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EngineeringLessonView {
    pub session_id: i64,
    pub subject_id: String,
    pub label: String,
    pub short_code: String,
    pub title: String,
    pub concept_slug: String,
    pub concept_title: String,
    pub category: String,
    pub curriculum: crate::db::CurriculumBrief,
    pub prerequisites: Vec<String>,
    pub session_index: i64,
    pub why_now: String,
    pub markdown: String,
    pub resources: Vec<Resource>,
    pub questions: Vec<ClassroomQuestionView>,
    pub exercise: Option<Exercise>,
    pub agent_used: String,
    pub prompt_profile: String,
    pub prompt_version: String,
    pub estimated_minutes: i64,
    pub status: String,
}

fn engineering_view(conn: &Connection, session_id: i64) -> Result<Option<EngineeringLessonView>> {
    let row = conn
        .query_row(
            "SELECT s.subject_id, p.label, p.short_code, s.payload_json,
                    s.agent_used, p.prompt_profile, s.prompt_version,
                    p.session_minutes, s.status
             FROM classroom_sessions s
             JOIN classroom_programs p ON p.subject_id = s.subject_id
             WHERE s.id = ?1",
            [session_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    row.map(
        |(
            subject_id,
            label,
            short_code,
            payload_json,
            agent_used,
            prompt_profile,
            prompt_version,
            minutes,
            status,
        )| {
            let stored: StoredEngineeringLesson =
                serde_json::from_str(&payload_json).map_err(|error| error.to_string())?;
            let concept = crate::db::get_concept(conn, stored.concept_id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "classroom concept no longer exists".to_string())?;
            let prerequisites_json: String = conn
                .query_row(
                    "SELECT prereqs_json FROM concepts WHERE id = ?1",
                    [concept.id],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            let session_index: i64 = conn
                .query_row(
                    "SELECT
                        (SELECT COUNT(*) FROM sessions
                         WHERE status = 'completed' AND focus = ?1)
                      + (SELECT COUNT(*) FROM classroom_sessions
                         WHERE status = 'completed' AND subject_id = ?1) + 1",
                    [&subject_id],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            let why_now = format!(
                "Session {session_index} advances the {} phase: {}",
                concept.curriculum.phase, concept.curriculum.learner_outcome
            );
            Ok(EngineeringLessonView {
                session_id,
                subject_id,
                label,
                short_code,
                title: stored.title,
                concept_slug: concept.slug,
                concept_title: stored.concept_title,
                category: stored.category,
                curriculum: concept.curriculum,
                prerequisites: serde_json::from_str(&prerequisites_json).unwrap_or_default(),
                session_index,
                why_now,
                markdown: stored.markdown,
                resources: stored.resources,
                questions: stored
                    .questions
                    .into_iter()
                    .map(|question| ClassroomQuestionView {
                        id: question.id,
                        prompt: question.prompt,
                        choices: question.choices,
                        section: question.section,
                        learning_objective: question.learning_objective,
                    })
                    .collect(),
                exercise: stored.exercise,
                agent_used,
                prompt_profile,
                prompt_version,
                estimated_minutes: minutes,
                status,
            })
        },
    )
    .transpose()
}

pub fn engineering_chat_context(
    conn: &Connection,
    session_id: i64,
) -> Result<crate::generator::CourseChatContext> {
    let (subject_id, payload_json) = conn
        .query_row(
            "SELECT subject_id, payload_json
             FROM classroom_sessions WHERE id = ?1",
            [session_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "classroom session not found".to_string())?;
    let stored: StoredEngineeringLesson =
        serde_json::from_str(&payload_json).map_err(|error| error.to_string())?;
    let concept = db::get_concept(conn, stored.concept_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "classroom chat concept not found".to_string())?;
    let exercise = stored
        .exercise
        .as_ref()
        .map(serde_json::to_string_pretty)
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_else(|| "(this course has no separate exercise)".into());
    Ok(crate::generator::CourseChatContext {
        title: stored.title,
        focus: subject_id,
        markdown: stored.markdown,
        learner_outcome: concept.curriculum.learner_outcome,
        cumulative_artifact: concept.curriculum.artifact,
        exercise,
    })
}

pub fn classroom_exercise(conn: &Connection, session_id: i64) -> Result<Option<db::ExerciseView>> {
    let payload_json: Option<String> = conn
        .query_row(
            "SELECT payload_json FROM classroom_sessions WHERE id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some(payload_json) = payload_json else {
        return Ok(None);
    };
    let stored: StoredEngineeringLesson =
        serde_json::from_str(&payload_json).map_err(|error| error.to_string())?;
    let draft =
        db::get_exercise_draft(conn, None, Some(session_id)).map_err(|error| error.to_string())?;
    let (completed, reflection) = db::get_exercise_completion(conn, None, Some(session_id))
        .map_err(|error| error.to_string())?;
    Ok(stored.exercise.map(|exercise| db::ExerciseView {
        course_id: None,
        classroom_session_id: Some(session_id),
        title: exercise.title,
        instructions: exercise.instructions,
        starter_code: exercise.starter_code,
        deliverable: exercise.deliverable,
        hints: exercise.hints,
        draft,
        completed,
        reflection,
    }))
}

fn active_engineering_for(
    conn: &Connection,
    subject_id: &str,
) -> Result<Option<EngineeringLessonView>> {
    let id = conn
        .query_row(
            "SELECT id FROM classroom_sessions
             WHERE subject_id = ?1 AND status = 'in_progress'
             ORDER BY id DESC LIMIT 1",
            [subject_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    id.map(|id| engineering_view(conn, id))
        .transpose()
        .map(|value| value.flatten())
}

pub fn active_engineering_session(
    conn: &Connection,
    subject_id: &str,
) -> Result<Option<EngineeringLessonView>> {
    active_engineering_for(conn, subject_id)
}

pub fn generation_profile(row: &ProgramRow) -> GenerationProfile {
    GenerationProfile {
        subject_id: row.subject_id.clone(),
        agent: row.agent.clone(),
        model: row.model.clone(),
        custom_bin: row.custom_agent_bin.clone(),
        prompt_version: format!("{}.{}", row.prompt_profile, row.prompt_version),
    }
}

/// Prepend the learner's stated goal (from the schedule planner) to a
/// subject's static prompt contract. The goal only nudges tone/emphasis —
/// it never overrides the contract's required subject lens or, for
/// languages, the frozen CEFR objective and assessment gates.
pub fn contract_with_goal(program: &ProgramRow, base_contract: &str) -> String {
    let goal = program.learning_goal.trim();
    if goal.is_empty() {
        base_contract.to_string()
    } else {
        format!("LEARNER GOAL FOR THIS CLASS: {goal}\n\n{base_contract}")
    }
}

pub async fn start_engineering_session(
    state: &AppState,
    subject_id: &str,
    slot_id: Option<i64>,
    revisit: bool,
) -> Result<EngineeringLessonView> {
    let (program, concept, dossier, contract) = {
        let conn = state.db.0.lock().unwrap();
        let spec = subject(subject_id)?;
        if spec.kind != SubjectKind::Engineering {
            return Err(format!("{subject_id} is not an engineering class"));
        }
        let program = program_row(&conn, subject_id)?;
        if !program.enabled {
            return Err(format!("{} is not enabled", program.label));
        }
        if let Some(active) = active_engineering_for(&conn, subject_id)? {
            return Ok(active);
        }
        if let Some(id) = slot_id {
            let owner: Option<String> = conn
                .query_row(
                    "SELECT subject_id FROM classroom_schedule_slots WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|error| error.to_string())?;
            if owner.as_deref() != Some(subject_id) {
                return Err("classroom slot does not belong to this subject".into());
            }
            let consumed: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM classroom_sessions
                     WHERE slot_id = ?1 AND session_date = ?2",
                    params![id, state.today()],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            if consumed > 0 {
                return Err("this class slot is already complete for today".into());
            }
        }
        let concept = {
            let drawn = if revisit {
                roulette::draw_completed(&conn, &state.today(), subject_id)
            } else {
                roulette::draw(&conn, &state.today(), subject_id)
            }
            .map_err(|error| error.to_string())?;
            drawn.ok_or_else(|| {
                if revisit {
                    format!("no completed modules to revisit in {}", program.label)
                } else {
                    format!(
                        "every module in {} is completed — use revisit or pick another subject",
                        program.label
                    )
                }
            })?
        };
        let dossier =
            mastery::build_dossier(&conn, &state.today(), subject_id).map_err(|e| e.to_string())?;
        (program, concept, dossier, spec.prompt)
    };
    let generation_profile = generation_profile(&program);
    let contract = contract_with_goal(&program, contract);
    let (course, source) = state
        .generator
        .generate_classroom_course(
            crate::generator::CourseRequest {
                title: &concept.title,
                category: &concept.category,
                dossier: &dossier,
                focus: subject_id,
                curriculum: &concept.curriculum,
            },
            &contract,
            &generation_profile,
        )
        .await
        .map_err(|error| error.to_string())?;
    insert_engineering_session(
        state,
        &program,
        EngineeringSessionMeta {
            concept_id: concept.id,
            concept_title: &concept.title,
            category: &concept.category,
            slot_id,
        },
        course,
        source,
    )
}

struct EngineeringSessionMeta<'a> {
    concept_id: i64,
    concept_title: &'a str,
    category: &'a str,
    slot_id: Option<i64>,
}

/// Map generated exit questions to stored questions, resolving each
/// `correct_answer` string to exactly one choice position. Fails closed: a
/// generated check whose correct answer matches zero or several choices is a
/// validation error, never a silently mis-graded session.
fn stored_questions(course: &GeneratedCourse) -> Result<Vec<StoredQuestion>> {
    let questions = course
        .exit_questions
        .iter()
        .enumerate()
        .map(|(index, question)| {
            let matches: Vec<usize> = question
                .choices
                .iter()
                .enumerate()
                .filter(|(_, choice)| choice.trim() == question.correct_answer.trim())
                .map(|(position, _)| position)
                .collect();
            let correct_index = match matches.as_slice() {
                [single] => *single,
                [] => {
                    return Err(format!(
                        "classroom course check {} has no matching correct answer",
                        index + 1
                    ));
                }
                _ => {
                    return Err(format!(
                        "classroom course check {} has an ambiguous correct answer",
                        index + 1
                    ));
                }
            };
            Ok(StoredQuestion {
                id: index + 1,
                prompt: question.prompt.clone(),
                choices: question.choices.clone(),
                correct_index,
                explanation: question.explanation.clone(),
                section: question.section.clone(),
                learning_objective: question.learning_objective.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    if questions.len() != 5 {
        return Err(format!(
            "classroom course produced {} validated checks; expected 5",
            questions.len()
        ));
    }
    Ok(questions)
}

fn insert_engineering_session(
    state: &AppState,
    program: &ProgramRow,
    meta: EngineeringSessionMeta<'_>,
    course: GeneratedCourse,
    source: String,
) -> Result<EngineeringLessonView> {
    let questions = stored_questions(&course)?;
    let stored = StoredEngineeringLesson {
        concept_id: meta.concept_id,
        concept_title: meta.concept_title.into(),
        category: meta.category.into(),
        title: course.title,
        markdown: course.markdown,
        resources: course.resources,
        questions,
        exercise: course.exercise,
        source: source.clone(),
    };
    let payload_json = serde_json::to_string(&stored).map_err(|error| error.to_string())?;
    let conn = state.db.0.lock().unwrap();
    if let Some(active) = active_engineering_for(&conn, &program.subject_id)? {
        return Ok(active);
    }
    conn.execute(
        "INSERT INTO classroom_sessions
            (subject_id, slot_id, session_date, status, title, payload_json,
             agent_used, prompt_version, started_at)
         VALUES (?1, ?2, ?3, 'in_progress', ?4, ?5, ?6, ?7, ?8)",
        params![
            program.subject_id,
            meta.slot_id,
            state.today(),
            stored.title,
            payload_json,
            source,
            format!("{}.{}", program.prompt_profile, program.prompt_version),
            language::now_iso(),
        ],
    )
    .map_err(|error| error.to_string())?;
    engineering_view(&conn, conn.last_insert_rowid())?
        .ok_or_else(|| "new classroom session could not be read".into())
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubmitEngineeringInput {
    pub session_id: i64,
    pub answers: Vec<usize>,
    #[serde(default)]
    pub reflection: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClassroomCorrectionView {
    pub question_id: usize,
    pub prompt: String,
    pub selected_answer: String,
    pub correct_answer: String,
    pub correct: bool,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EngineeringSessionResult {
    pub session_id: i64,
    pub subject_id: String,
    pub passed: bool,
    pub score: f64,
    pub corrections: Vec<ClassroomCorrectionView>,
}

pub fn submit_engineering_session(
    conn: &Connection,
    input: &SubmitEngineeringInput,
    today: &str,
) -> Result<EngineeringSessionResult> {
    let transaction = conn
        .unchecked_transaction()
        .map_err(|error| error.to_string())?;
    let conn = &*transaction;
    let row = conn
        .query_row(
            "SELECT subject_id, status, payload_json FROM classroom_sessions WHERE id = ?1",
            [input.session_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(|error| error.to_string())?;
    let (subject_id, status, payload_json) = row;
    if status != "in_progress" {
        return Err("this classroom session is already finished".into());
    }
    let stored: StoredEngineeringLesson =
        serde_json::from_str(&payload_json).map_err(|error| error.to_string())?;
    if input.answers.len() != stored.questions.len() {
        return Err("answer every classroom knowledge check".into());
    }
    let mut correct_count = 0;
    let mut corrections = Vec::new();
    for (index, question) in stored.questions.iter().enumerate() {
        let selected = input.answers[index];
        if selected >= question.choices.len() {
            return Err(format!("answer {} is invalid", index + 1));
        }
        let correct = selected == question.correct_index;
        correct_count += usize::from(correct);
        corrections.push(ClassroomCorrectionView {
            question_id: question.id,
            prompt: question.prompt.clone(),
            selected_answer: question.choices[selected].clone(),
            correct_answer: question.choices[question.correct_index].clone(),
            correct,
            explanation: question.explanation.clone(),
        });
    }
    let score = correct_count as f64 / stored.questions.len() as f64;
    for (question, correction) in stored.questions.iter().zip(&corrections) {
        let misconception = if correction.correct {
            String::new()
        } else {
            format!(
                "Selected “{}” instead of “{}”. {}",
                correction.selected_answer, correction.correct_answer, correction.explanation
            )
        };
        conn.execute(
            "INSERT INTO classroom_exit_attempts
                (session_id, concept_id, question_id, section, learning_objective,
                 misconception, correct, attempted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                input.session_id,
                stored.concept_id,
                question.id as i64,
                question.section,
                question.learning_objective,
                misconception,
                correction.correct as i64,
                language::now_iso(),
            ],
        )
        .map_err(|error| error.to_string())?;
    }
    mastery::record_course_read(conn, stored.concept_id, today).map_err(|e| e.to_string())?;
    mastery::record_quiz_outcome(conn, stored.concept_id, today, score)
        .map_err(|e| e.to_string())?;
    let response_json = serde_json::json!({
        "answers": input.answers,
        "reflection": input.reflection.trim(),
    })
    .to_string();
    conn.execute(
        "UPDATE classroom_sessions
         SET status = 'completed', score = ?2, response_json = ?3,
             completed_at = ?4
         WHERE id = ?1",
        params![input.session_id, score, response_json, language::now_iso(),],
    )
    .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(EngineeringSessionResult {
        session_id: input.session_id,
        subject_id,
        passed: score >= 0.8,
        score,
        corrections,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ActiveClassroomSessionView {
    pub session_id: i64,
    pub subject_id: String,
    pub kind: String,
    pub label: String,
    pub title: String,
}

pub fn active_sessions(conn: &Connection) -> Result<Vec<ActiveClassroomSessionView>> {
    let mut active = Vec::new();
    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.subject_id, p.label, s.title
             FROM classroom_sessions s
             JOIN classroom_programs p ON p.subject_id = s.subject_id
             WHERE s.status = 'in_progress'
             ORDER BY s.id",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ActiveClassroomSessionView {
                session_id: row.get(0)?,
                subject_id: row.get(1)?,
                kind: "engineering".into(),
                label: row.get(2)?,
                title: row.get(3)?,
            })
        })
        .map_err(|error| error.to_string())?;
    for row in rows {
        active.push(row.map_err(|error| error.to_string())?);
    }
    for language in language::active_summaries(conn)? {
        active.push(ActiveClassroomSessionView {
            session_id: language.session_id,
            subject_id: language.language,
            kind: "language".into(),
            label: language.label,
            title: language.title,
        });
    }
    Ok(active)
}

pub fn prompt_contracts_are_isolated() -> Result<()> {
    let mut seen = HashMap::new();
    for spec in SUBJECTS {
        if !spec
            .prompt
            .contains(&format!("PROMPT PROFILE: {}.v1", spec.prompt_profile))
        {
            return Err(format!("{} prompt has the wrong version marker", spec.id));
        }
        if seen.insert(spec.prompt, spec.id).is_some() {
            return Err(format!("{} reuses another subject prompt", spec.id));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::ExitCheck;

    fn question(correct_answer: &str, choices: &[&str]) -> ExitCheck {
        ExitCheck {
            prompt: "what is it?".into(),
            choices: choices.iter().map(|choice| choice.to_string()).collect(),
            correct_answer: correct_answer.into(),
            explanation: "because".into(),
            section: "## Core mechanics".into(),
            learning_objective: "explain the mechanism".into(),
        }
    }

    fn course(questions: Vec<ExitCheck>) -> GeneratedCourse {
        GeneratedCourse {
            title: "test".into(),
            markdown: String::new(),
            resources: vec![],
            key_takeaways: vec![],
            exit_questions: questions,
            exercise: None,
        }
    }

    #[test]
    fn stored_questions_resolve_the_correct_choice_position() {
        let stored = stored_questions(&course(vec![
            question("b", &["a", "b", "c"]),
            question("c", &["a", "b", "c"]),
            question("a", &["a", "b", "c"]),
            question("b", &["a", "b", "c"]),
            question("a", &["a", "b", "c"]),
        ]))
        .expect("matching answers must map");
        let positions: Vec<usize> = stored.iter().map(|q| q.correct_index).collect();
        assert_eq!(positions, vec![1, 2, 0, 1, 0]);
    }

    #[test]
    fn stored_questions_trim_before_matching() {
        let stored = stored_questions(&course(vec![
            question(" b ", &["a", "b", "c"]),
            question("c", &["a", "b", "c"]),
            question("a", &["a", "b", "c"]),
            question("b", &["a", "b", "c"]),
            question("a", &["a", "b", "c"]),
        ]))
        .expect("whitespace-padded answers must match");
        assert_eq!(stored[0].correct_index, 1);
    }

    #[test]
    fn stored_questions_fail_closed_when_no_choice_matches() {
        let error = stored_questions(&course(vec![
            question("z", &["a", "b", "c"]),
            question("b", &["a", "b", "c"]),
            question("a", &["a", "b", "c"]),
            question("b", &["a", "b", "c"]),
            question("a", &["a", "b", "c"]),
        ]))
        .expect_err("unmatched answer must be a validation error");
        assert!(
            error.contains("check 1 has no matching correct answer"),
            "{error}"
        );
    }

    #[test]
    fn stored_questions_fail_closed_when_answer_is_ambiguous() {
        let error = stored_questions(&course(vec![
            question("b", &["a", "b", "b"]),
            question("b", &["a", "b", "c"]),
            question("a", &["a", "b", "c"]),
            question("b", &["a", "b", "c"]),
            question("a", &["a", "b", "c"]),
        ]))
        .expect_err("duplicated choice must be a validation error");
        assert!(
            error.contains("check 1 has an ambiguous correct answer"),
            "{error}"
        );
    }

    #[test]
    fn stored_questions_require_exactly_five() {
        let error = stored_questions(&course(vec![
            question("b", &["a", "b", "c"]),
            question("a", &["a", "b", "c"]),
        ]))
        .expect_err("four checks are not a classroom course");
        assert!(error.contains("expected 5"), "{error}");
    }
}

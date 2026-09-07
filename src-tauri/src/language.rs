use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, Timelike, Weekday};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

const GERMAN_SEED: &str = include_str!("../seed/languages/german.json");
const ITALIAN_SEED: &str = include_str!("../seed/languages/italian.json");

pub const LANGUAGES: &[&str] = &["german", "italian"];
pub const LEVELS: &[&str] = &["A1", "A2", "B1", "B2"];
pub const STRANDS: &[&str] = &[
    "listening",
    "reading",
    "spoken_interaction",
    "spoken_production",
    "writing",
    "grammar",
    "vocabulary_pragmatics",
];

type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfficialSource {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocabularyItem {
    pub term: String,
    pub meaning: String,
    pub example: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phrase {
    pub target: String,
    pub translation: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueLine {
    pub speaker: String,
    pub target: String,
    pub translation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SeedCheck {
    prompt: String,
    choices: Vec<String>,
    correct_index: usize,
    explanation: String,
    strand: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FoundationBlock {
    id: String,
    title: String,
    explanation: String,
    practice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UnitSpec {
    slug: String,
    title: String,
    scenario: String,
    can_do: String,
    strands: Vec<String>,
    #[serde(default)]
    foundations: Vec<FoundationBlock>,
    grammar: Vec<String>,
    vocabulary: Vec<VocabularyItem>,
    phrases: Vec<Phrase>,
    dialogue: Vec<DialogueLine>,
    pronunciation: Vec<String>,
    pragmatics: String,
    culture: String,
    speaking_prompt: String,
    writing_prompt: String,
    checks: Vec<SeedCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LevelSpec {
    level: String,
    sessions_per_unit: usize,
    units: Vec<UnitSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Curriculum {
    language: String,
    label: String,
    native_label: String,
    official_sources: Vec<OfficialSource>,
    levels: Vec<LevelSpec>,
}

static GERMAN: OnceLock<Curriculum> = OnceLock::new();
static ITALIAN: OnceLock<Curriculum> = OnceLock::new();

fn valid_language(language: &str) -> bool {
    LANGUAGES.contains(&language)
}

fn valid_level(level: &str) -> bool {
    LEVELS.contains(&level)
}

fn curriculum(language: &str) -> Result<&'static Curriculum> {
    match language {
        "german" => Ok(GERMAN.get_or_init(|| {
            serde_json::from_str(GERMAN_SEED).expect("bundled German curriculum must be valid")
        })),
        "italian" => Ok(ITALIAN.get_or_init(|| {
            serde_json::from_str(ITALIAN_SEED).expect("bundled Italian curriculum must be valid")
        })),
        _ => Err(format!("unsupported language: {language}")),
    }
}

fn level_spec<'a>(curriculum: &'a Curriculum, level: &str) -> Result<&'a LevelSpec> {
    curriculum
        .levels
        .iter()
        .find(|candidate| candidate.level == level)
        .ok_or_else(|| format!("curriculum is missing {level}"))
}

pub fn validate_curriculum(language: &str) -> Result<()> {
    let curriculum = curriculum(language)?;
    if curriculum.language != language {
        return Err("curriculum language does not match its file".into());
    }
    if curriculum.levels.len() != LEVELS.len() {
        return Err(format!("{language} must have {} levels", LEVELS.len()));
    }
    if curriculum
        .official_sources
        .iter()
        .any(|source| !source.url.starts_with("https://"))
    {
        return Err(format!("{language} has a non-HTTPS official source"));
    }
    let mut slugs = HashSet::new();
    for expected in LEVELS {
        let level = level_spec(curriculum, expected)?;
        if level.units.len() != 10 || level.sessions_per_unit == 0 {
            return Err(format!(
                "{language} {expected} needs 10 units and a non-zero session count"
            ));
        }
        for unit in &level.units {
            if !slugs.insert(unit.slug.as_str()) {
                return Err(format!("duplicate language unit slug {}", unit.slug));
            }
            if unit.vocabulary.len() != 8
                || unit.phrases.len() != 6
                || unit.dialogue.len() != 6
                || unit.pronunciation.len() != 2
                || unit.checks.len() != 5
                || unit.strands.len() < 3
            {
                return Err(format!("{} does not meet the content contract", unit.slug));
            }
            if unit
                .strands
                .iter()
                .chain(unit.checks.iter().map(|check| &check.strand))
                .any(|strand| !STRANDS.contains(&strand.as_str()))
            {
                return Err(format!("{} contains an unknown skill strand", unit.slug));
            }
            if unit
                .checks
                .iter()
                .any(|check| check.choices.len() != 4 || check.correct_index >= check.choices.len())
            {
                return Err(format!("{} contains an invalid knowledge check", unit.slug));
            }
            if unit.foundations.iter().any(|block| {
                block.id.trim().is_empty()
                    || block.title.trim().is_empty()
                    || block.explanation.split_whitespace().count() < 8
                    || block.practice.split_whitespace().count() < 6
            }) {
                return Err(format!(
                    "{} contains an incomplete first-principles foundation",
                    unit.slug
                ));
            }
        }
        if *expected == "A1" {
            let alphabet = &level.units[0].foundations;
            if !alphabet.iter().any(|block| block.id == "alphabet")
                || !alphabet.iter().any(|block| block.id == "sound-spelling")
            {
                return Err(format!(
                    "{language} A1 must begin with alphabet and sound-spelling foundations"
                ));
            }
            let numbers = &level.units[1].foundations;
            if !numbers.iter().any(|block| block.id == "counting")
                || !numbers.iter().any(|block| block.id == "number-building")
            {
                return Err(format!(
                    "{language} A1 unit two must teach counting and number construction"
                ));
            }
        }
    }
    Ok(())
}

pub fn initialize(conn: &Connection, today: &str) -> Result<()> {
    for language in LANGUAGES {
        validate_curriculum(language)?;
        conn.execute(
            "INSERT OR IGNORE INTO language_programs
                (language, enabled, start_level, current_level, target_level, start_date,
                 weekly_minutes, session_minutes, preferred, updated_at)
             VALUES (?1, 0, 'A1', 'A1', 'A2', ?2, 210, 30, ?3, ?4)",
            params![
                language,
                today,
                if *language == "german" { 1 } else { 0 },
                now_iso()
            ],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct MilestoneView {
    pub level: String,
    pub target_date: String,
    pub reached: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillScoreView {
    pub id: String,
    pub label: String,
    pub score: f64,
    pub encounters: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LanguageProgramView {
    pub language: String,
    pub label: String,
    pub native_label: String,
    pub enabled: bool,
    pub start_level: String,
    pub current_level: String,
    pub target_level: String,
    pub start_date: String,
    pub target_date: String,
    pub weekly_minutes: i64,
    pub session_minutes: i64,
    pub completed_steps: i64,
    pub required_steps: i64,
    pub progress: f64,
    pub total_completed_steps: i64,
    pub total_required_steps: i64,
    pub recommended_weekly_minutes: i64,
    pub pace_status: String,
    pub pace_message: String,
    pub milestones: Vec<MilestoneView>,
    pub skills: Vec<SkillScoreView>,
    pub official_sources: Vec<OfficialSource>,
}

#[derive(Debug, Clone)]
struct ProgramRow {
    language: String,
    enabled: bool,
    start_level: String,
    current_level: String,
    target_level: String,
    start_date: String,
    weekly_minutes: i64,
    session_minutes: i64,
}

fn program_row(conn: &Connection, language: &str) -> Result<ProgramRow> {
    conn.query_row(
        "SELECT language, enabled, start_level, current_level, target_level,
                start_date, weekly_minutes, session_minutes
         FROM language_programs WHERE language = ?1",
        [language],
        |row| {
            Ok(ProgramRow {
                language: row.get(0)?,
                enabled: row.get::<_, i64>(1)? != 0,
                start_level: row.get(2)?,
                current_level: row.get(3)?,
                target_level: row.get(4)?,
                start_date: row.get(5)?,
                weekly_minutes: row.get(6)?,
                session_minutes: row.get(7)?,
            })
        },
    )
    .map_err(|error| error.to_string())
}

fn target_offset_days(level: &str) -> i64 {
    match level {
        "A1" => 30,
        "A2" => 90,
        "B1" => 270,
        "B2" => 540,
        _ => 90,
    }
}

fn minimum_total_weekly_minutes(language: &str, target_level: &str) -> i64 {
    match (language, target_level) {
        // Lower-bound guided-hour ranges spread across the requested target
        // windows. These are planning warnings, not certification promises.
        ("german", "A1") => 840,
        ("german", "A2") => 700,
        ("german", "B1") => 560,
        ("german", "B2") => 480,
        ("italian", "A1") => 630,
        ("italian", "A2") => 420,
        ("italian", "B1") => 390,
        ("italian", "B2") => 350,
        _ => 420,
    }
}

fn level_index(level: &str) -> usize {
    LEVELS
        .iter()
        .position(|candidate| *candidate == level)
        .unwrap_or(0)
}

fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn parse_date(date: &str) -> NaiveDate {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap_or_else(|_| Local::now().date_naive())
}

fn level_progress(conn: &Connection, language: &str, level: &LevelSpec) -> Result<(i64, i64)> {
    let required = (level.units.len() * level.sessions_per_unit) as i64;
    let mut completed = 0;
    for unit in &level.units {
        let phase: i64 = conn
            .query_row(
                "SELECT phase_completed FROM language_unit_progress
                 WHERE language = ?1 AND unit_slug = ?2",
                params![language, unit.slug],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .unwrap_or(0);
        completed += phase.clamp(0, level.sessions_per_unit as i64);
    }
    Ok((completed, required))
}

/// Every session of every unit in `level` has been completed.
pub fn level_complete(conn: &Connection, language: &str, level: &str) -> Result<bool> {
    let curriculum = curriculum(language)?;
    let spec = level_spec(curriculum, level)?;
    let (completed, required) = level_progress(conn, language, spec)?;
    Ok(completed >= required)
}

fn skills_for(conn: &Connection, language: &str) -> Result<Vec<SkillScoreView>> {
    let mut scores = HashMap::new();
    let mut stmt = conn
        .prepare(
            "SELECT strand, score_ema, encounters
             FROM language_skill_scores WHERE language = ?1",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([language], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    for row in rows {
        let (strand, score, encounters) = row.map_err(|error| error.to_string())?;
        scores.insert(strand, (score, encounters));
    }
    Ok(STRANDS
        .iter()
        .map(|strand| {
            let (score, encounters) = scores.get(*strand).copied().unwrap_or((0.0, 0));
            SkillScoreView {
                id: (*strand).to_string(),
                label: strand
                    .split('_')
                    .map(capitalize)
                    .collect::<Vec<_>>()
                    .join(" "),
                score,
                encounters,
            }
        })
        .collect())
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub fn program_views(conn: &Connection, today: &str) -> Result<Vec<LanguageProgramView>> {
    LANGUAGES
        .iter()
        .map(|language| program_view(conn, language, today))
        .collect()
}

pub fn program_view(conn: &Connection, language: &str, today: &str) -> Result<LanguageProgramView> {
    let row = program_row(conn, language)?;
    let curriculum = curriculum(language)?;
    let current = level_spec(curriculum, &row.current_level)?;
    let (completed_steps, required_steps) = level_progress(conn, language, current)?;
    let start_index = level_index(&row.start_level);
    let target_index = level_index(&row.target_level).max(start_index);
    let mut total_completed = 0;
    let mut total_required = 0;
    for level in &curriculum.levels[start_index..=target_index] {
        let (completed, required) = level_progress(conn, language, level)?;
        total_completed += completed;
        total_required += required;
    }

    let start = parse_date(&row.start_date);
    let skills = skills_for(conn, language)?;
    let current_gate_ready = skills
        .iter()
        .all(|skill| skill.encounters > 0 && skill.score >= 0.60);
    let milestones = LEVELS
        .iter()
        .map(|level| MilestoneView {
            level: (*level).to_string(),
            target_date: format_date(start + Duration::days(target_offset_days(level))),
            reached: level_index(&row.current_level) > level_index(level)
                || (*level == "B2" && completed_steps >= required_steps && current_gate_ready),
        })
        .collect::<Vec<_>>();
    let target_date = format_date(start + Duration::days(target_offset_days(&row.target_level)));
    let target_days = target_offset_days(&row.target_level).max(1);
    let app_cadence_weekly =
        ((total_required as f64 * row.session_minutes as f64 / target_days as f64) * 7.0).ceil()
            as i64;
    let recommended_weekly =
        app_cadence_weekly.max(minimum_total_weekly_minutes(language, &row.target_level));
    let elapsed = (parse_date(today) - start).num_days().max(0);
    let expected_progress = (elapsed as f64 / target_days as f64).clamp(0.0, 1.0);
    let actual_progress = if total_required == 0 {
        0.0
    } else {
        total_completed as f64 / total_required as f64
    };
    let commitment_ok = row.weekly_minutes >= recommended_weekly;
    let cadence_ok = actual_progress + 0.08 >= expected_progress;
    let pace_status = if commitment_ok && cadence_ok {
        "on_track"
    } else if cadence_ok {
        "commitment_gap"
    } else {
        "behind"
    };
    let pace_message = match pace_status {
        "on_track" => format!(
            "Your total weekly commitment supports the {} stretch target. Keep varied real listening and live speaking in the plan.",
            row.target_level
        ),
        "commitment_gap" => format!(
            "The {} timeline needs roughly {recommended_weekly} total practice minutes each week at the lower-bound pace; you configured {}. App sessions are only one part.",
            row.target_level,
            row.weekly_minutes
        ),
        _ => "The target date is a stretch at the current evidence rate. Add a slot or move the date; the app will not advance the level on time alone.".to_string(),
    };

    Ok(LanguageProgramView {
        language: row.language,
        label: curriculum.label.clone(),
        native_label: curriculum.native_label.clone(),
        enabled: row.enabled,
        start_level: row.start_level,
        current_level: row.current_level,
        target_level: row.target_level,
        start_date: row.start_date,
        target_date,
        weekly_minutes: row.weekly_minutes,
        session_minutes: row.session_minutes,
        completed_steps,
        required_steps,
        progress: if required_steps == 0 {
            0.0
        } else {
            completed_steps as f64 / required_steps as f64
        },
        total_completed_steps: total_completed,
        total_required_steps: total_required,
        recommended_weekly_minutes: recommended_weekly,
        pace_status: pace_status.to_string(),
        pace_message,
        milestones,
        skills,
        official_sources: curriculum.official_sources.clone(),
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigureProgramInput {
    pub language: String,
    pub enabled: bool,
    pub start_level: String,
    pub target_level: String,
    pub weekly_minutes: i64,
    pub session_minutes: i64,
}

pub fn configure_program(
    conn: &Connection,
    input: &ConfigureProgramInput,
    today: &str,
) -> Result<()> {
    if !valid_language(&input.language) {
        return Err(format!("unsupported language: {}", input.language));
    }
    if !valid_level(&input.start_level) || !valid_level(&input.target_level) {
        return Err("language level must be A1, A2, B1, or B2".into());
    }
    if level_index(&input.target_level) < level_index(&input.start_level) {
        return Err("target level cannot be below the starting level".into());
    }
    if !(60..=2_100).contains(&input.weekly_minutes) {
        return Err("weekly practice must be between 60 and 2100 minutes".into());
    }
    if !(15..=90).contains(&input.session_minutes) {
        return Err("session length must be between 15 and 90 minutes".into());
    }
    let active: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM language_sessions
             WHERE language = ?1 AND status = 'in_progress'",
            [&input.language],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !input.enabled && active > 0 {
        return Err("finish the active language session before disabling its program".into());
    }
    let encounters: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM language_sessions WHERE language = ?1",
            [&input.language],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let existing = program_row(conn, &input.language)?;
    if encounters > 0 && level_index(&input.target_level) < level_index(&existing.current_level) {
        return Err("target level cannot be below the demonstrated current level".into());
    }
    let current_level = if encounters == 0 {
        input.start_level.clone()
    } else {
        existing.current_level
    };
    let start_date = if encounters == 0 {
        today.to_string()
    } else {
        existing.start_date
    };
    conn.execute(
        "UPDATE language_programs
         SET enabled = ?2, start_level = ?3, current_level = ?4, target_level = ?5,
             start_date = ?6, weekly_minutes = ?7, session_minutes = ?8, updated_at = ?9
         WHERE language = ?1",
        params![
            input.language,
            i64::from(input.enabled),
            input.start_level,
            current_level,
            input.target_level,
            start_date,
            input.weekly_minutes,
            input.session_minutes,
            now_iso()
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpsertSlotInput {
    pub id: Option<i64>,
    pub language: String,
    pub hour: u32,
    pub minute: u32,
    pub weekdays: Vec<u8>,
    pub enabled: bool,
}

pub fn upsert_slot(conn: &Connection, input: &UpsertSlotInput) -> Result<i64> {
    if !valid_language(&input.language) {
        return Err(format!("unsupported language: {}", input.language));
    }
    if input.hour > 23 || input.minute > 59 {
        return Err("schedule time is invalid".into());
    }
    let mut weekdays = input.weekdays.clone();
    weekdays.sort_unstable();
    weekdays.dedup();
    if weekdays.is_empty() || weekdays.iter().any(|day| !(1..=7).contains(day)) {
        return Err("select at least one valid weekday".into());
    }
    let weekdays_json = serde_json::to_string(&weekdays).map_err(|error| error.to_string())?;
    if let Some(id) = input.id {
        let changed = conn
            .execute(
                "UPDATE language_schedule_slots
                 SET language = ?2, hour = ?3, minute = ?4, weekdays_json = ?5, enabled = ?6
                 WHERE id = ?1",
                params![
                    id,
                    input.language,
                    input.hour,
                    input.minute,
                    weekdays_json,
                    i64::from(input.enabled)
                ],
            )
            .map_err(|error| error.to_string())?;
        if changed == 0 {
            return Err("language schedule slot was not found".into());
        }
        Ok(id)
    } else {
        conn.execute(
            "INSERT INTO language_schedule_slots
                (language, hour, minute, weekdays_json, enabled, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                input.language,
                input.hour,
                input.minute,
                weekdays_json,
                i64::from(input.enabled),
                now_iso()
            ],
        )
        .map_err(|error| {
            if error.to_string().contains("UNIQUE constraint failed") {
                "that language already has a slot at this time".to_string()
            } else {
                error.to_string()
            }
        })?;
        Ok(conn.last_insert_rowid())
    }
}

pub fn delete_slot(conn: &Connection, id: i64) -> Result<()> {
    let active: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM language_sessions
             WHERE slot_id = ?1 AND status = 'in_progress'",
            [id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if active > 0 {
        return Err("finish the active session before deleting its slot".into());
    }
    conn.execute("DELETE FROM language_schedule_slots WHERE id = ?1", [id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct LanguageSlotView {
    pub id: i64,
    pub language: String,
    pub label: String,
    pub hour: u32,
    pub minute: u32,
    pub weekdays: Vec<u8>,
    pub enabled: bool,
    pub owed: bool,
    pub next_fire_at: String,
    pub in_progress: bool,
}

#[derive(Debug, Clone)]
struct SlotRow {
    id: i64,
    language: String,
    hour: u32,
    minute: u32,
    weekdays: Vec<u8>,
    enabled: bool,
    program_enabled: bool,
}

fn slot_rows(conn: &Connection) -> Result<Vec<SlotRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.language, s.hour, s.minute, s.weekdays_json, s.enabled, p.enabled
             FROM language_schedule_slots s
             JOIN language_programs p ON p.language = s.language
             ORDER BY s.hour, s.minute, s.id",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let weekdays_json: String = row.get(4)?;
            Ok(SlotRow {
                id: row.get(0)?,
                language: row.get(1)?,
                hour: row.get::<_, i64>(2)? as u32,
                minute: row.get::<_, i64>(3)? as u32,
                weekdays: serde_json::from_str(&weekdays_json).unwrap_or_default(),
                enabled: row.get::<_, i64>(5)? != 0,
                program_enabled: row.get::<_, i64>(6)? != 0,
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

fn slot_session_state(conn: &Connection, slot_id: i64, today: &str) -> Result<(bool, bool)> {
    conn.query_row(
        "SELECT status FROM language_sessions
         WHERE slot_id = ?1 AND session_date = ?2
         ORDER BY id DESC LIMIT 1",
        params![slot_id, today],
        |row| row.get::<_, String>(0),
    )
    .optional()
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

pub fn slot_views(
    conn: &Connection,
    today: &str,
    debug_day: bool,
) -> Result<Vec<LanguageSlotView>> {
    let paused = matches!(
        crate::db::get_config(conn, "schedule_paused"),
        Ok(Some(value)) if value == "1"
    );
    let now = Local::now().naive_local();
    slot_rows(conn)?
        .into_iter()
        .map(|slot| {
            let (consumed, in_progress) = slot_session_state(conn, slot.id, today)?;
            let available = slot.enabled && slot.program_enabled && !paused;
            let owed = available
                && !consumed
                && (debug_day || slot_due_at(slot.hour, slot.minute, &slot.weekdays, now, false));
            let label = curriculum(&slot.language)?.label.clone();
            let next_fire = next_fire_at(&slot, now, consumed);
            Ok(LanguageSlotView {
                id: slot.id,
                language: slot.language,
                label,
                hour: slot.hour,
                minute: slot.minute,
                weekdays: slot.weekdays.clone(),
                enabled: slot.enabled,
                owed,
                next_fire_at: next_fire,
                in_progress,
            })
        })
        .collect()
}

pub fn schedule_times(conn: &Connection) -> Result<Vec<(u32, u32)>> {
    let mut times = slot_rows(conn)?
        .into_iter()
        .filter(|slot| slot.enabled && slot.program_enabled)
        .map(|slot| (slot.hour, slot.minute))
        .collect::<Vec<_>>();
    times.sort_unstable();
    times.dedup();
    Ok(times)
}

pub fn all_schedule_times(conn: &Connection) -> Result<Vec<(u32, u32)>> {
    let primary_hour = crate::db::get_config(conn, "schedule_hour")
        .map_err(|error| error.to_string())?
        .and_then(|value| value.parse().ok())
        .unwrap_or(9);
    let primary_minute = crate::db::get_config(conn, "schedule_minute")
        .map_err(|error| error.to_string())?
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let mut times = vec![(primary_hour, primary_minute)];
    times.extend(schedule_times(conn)?);
    times.sort_unstable();
    times.dedup();
    Ok(times)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredQuestion {
    pub(crate) id: usize,
    pub(crate) prompt: String,
    pub(crate) choices: Vec<String>,
    pub(crate) correct_index: usize,
    pub(crate) explanation: String,
    pub(crate) strand: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredLesson {
    pub(crate) title: String,
    pub(crate) scenario: String,
    pub(crate) can_do: String,
    pub(crate) phase_label: String,
    pub(crate) markdown: String,
    pub(crate) phrases: Vec<Phrase>,
    pub(crate) dialogue: Vec<DialogueLine>,
    pub(crate) questions: Vec<StoredQuestion>,
    pub(crate) speaking_prompt: String,
    pub(crate) writing_prompt: String,
    pub(crate) listen_text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LanguageQuestionView {
    pub id: usize,
    pub prompt: String,
    pub choices: Vec<String>,
    pub strand: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LanguageLessonView {
    pub session_id: i64,
    pub language: String,
    pub label: String,
    pub native_label: String,
    pub level: String,
    pub unit_slug: String,
    pub phase: i64,
    pub phase_label: String,
    pub title: String,
    pub scenario: String,
    pub can_do: String,
    pub markdown: String,
    pub phrases: Vec<Phrase>,
    pub dialogue: Vec<DialogueLine>,
    pub questions: Vec<LanguageQuestionView>,
    pub speaking_prompt: String,
    pub writing_prompt: String,
    pub listen_text: String,
    pub estimated_minutes: i64,
    pub status: String,
}

fn phase_label(phase: i64) -> &'static str {
    match (phase - 1).rem_euclid(7) {
        0 => "notice and understand",
        1 => "form and meaning",
        2 => "guided interaction",
        3 => "listening transfer",
        4 => "written production",
        5 => "spoken production",
        _ => "integrated retrieval",
    }
}

fn phase_guidance(phase: i64) -> &'static str {
    match (phase - 1).rem_euclid(7) {
        0 => "Read the dialogue for the situation first. Notice recurring chunks before analysing individual words.",
        1 => "Compare the examples and say what changes. Build the grammar rule from the pattern, then verify it against the notes.",
        2 => "Treat every model line as one turn in a real exchange. Substitute your own details and answer aloud.",
        3 => "Listen without reading once, write the words you catch, then listen again with the transcript.",
        4 => "Use the target phrases to produce a short useful message. Prefer clear, level-appropriate language over translation word by word.",
        5 => "Rehearse once, then speak without reading. Record yourself if possible and check whether the task is understandable.",
        _ => "Retrieve the language in a changed situation. Explain why each answer fits before checking the feedback.",
    }
}

fn target_language_name(language: &str) -> &'static str {
    match language {
        "german" => "German",
        "italian" => "Italian",
        _ => "the target language",
    }
}

fn build_questions(unit: &UnitSpec, phase: i64) -> Vec<StoredQuestion> {
    let offset = (phase.max(1) as usize - 1) % unit.vocabulary.len();
    let vocab = &unit.vocabulary[offset];
    let reverse = &unit.vocabulary[(offset + 3) % unit.vocabulary.len()];
    let phrase = &unit.phrases[offset % unit.phrases.len()];

    let mut meaning_choices = unit
        .vocabulary
        .iter()
        .skip(offset + 1)
        .chain(unit.vocabulary.iter())
        .map(|item| item.meaning.clone())
        .filter(|meaning| meaning != &vocab.meaning)
        .take(3)
        .collect::<Vec<_>>();
    meaning_choices.insert(offset % 4, vocab.meaning.clone());
    let meaning_correct = offset % 4;

    let mut term_choices = unit
        .vocabulary
        .iter()
        .skip(offset + 2)
        .chain(unit.vocabulary.iter())
        .map(|item| item.term.clone())
        .filter(|term| term != &reverse.term)
        .take(3)
        .collect::<Vec<_>>();
    let term_correct = (offset + 1) % 4;
    term_choices.insert(term_correct, reverse.term.clone());

    let phrase_distractors = unit
        .phrases
        .iter()
        .filter(|candidate| candidate.target != phrase.target)
        .take(3)
        .map(|candidate| candidate.translation.clone())
        .collect::<Vec<_>>();
    let phrase_correct = (offset + 2) % 4;
    let mut phrase_choices = phrase_distractors;
    phrase_choices.insert(phrase_correct, phrase.translation.clone());

    let curated_a = &unit.checks[offset % unit.checks.len()];
    let curated_b = &unit.checks[(offset + 2) % unit.checks.len()];
    vec![
        StoredQuestion {
            id: 1,
            prompt: format!("In this scenario, what does “{}” mean?", vocab.term),
            choices: meaning_choices,
            correct_index: meaning_correct,
            explanation: format!("{} Example: {}", vocab.meaning, vocab.example),
            strand: "vocabulary_pragmatics".into(),
        },
        StoredQuestion {
            id: 2,
            prompt: format!(
                "Which target-language expression means “{}”?",
                reverse.meaning
            ),
            choices: term_choices,
            correct_index: term_correct,
            explanation: format!("The expression is “{}”. {}", reverse.term, reverse.example),
            strand: "vocabulary_pragmatics".into(),
        },
        StoredQuestion {
            id: 3,
            prompt: format!("Choose the best meaning of “{}”.", phrase.target),
            choices: phrase_choices,
            correct_index: phrase_correct,
            explanation: format!("{} {}", phrase.translation, phrase.note),
            strand: "reading".into(),
        },
        StoredQuestion {
            id: 4,
            prompt: curated_a.prompt.clone(),
            choices: curated_a.choices.clone(),
            correct_index: curated_a.correct_index,
            explanation: curated_a.explanation.clone(),
            strand: curated_a.strand.clone(),
        },
        StoredQuestion {
            id: 5,
            prompt: curated_b.prompt.clone(),
            choices: curated_b.choices.clone(),
            correct_index: curated_b.correct_index,
            explanation: curated_b.explanation.clone(),
            strand: curated_b.strand.clone(),
        },
    ]
}

fn build_lesson(language: &str, level: &str, unit: &UnitSpec, phase: i64) -> StoredLesson {
    let foundations = if unit.foundations.is_empty() {
        String::new()
    } else {
        let blocks = unit
            .foundations
            .iter()
            .map(|block| {
                format!(
                    "### {}\n\n{}\n\n**Deliberate practice:** {}",
                    block.title, block.explanation, block.practice
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        format!(
            "## First principles foundation\n\nStart here before memorising the scenario. Build the smallest reliable pieces first, then combine them into useful language.\n\n{blocks}\n\n"
        )
    };
    let vocabulary = unit
        .vocabulary
        .iter()
        .map(|item| {
            format!(
                "| **{}** | {} | _{}_ |",
                item.term, item.meaning, item.example
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let phrases = unit
        .phrases
        .iter()
        .map(|phrase| {
            format!(
                "- **{}** — {}  \n  _{}_",
                phrase.target, phrase.translation, phrase.note
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let dialogue = unit
        .dialogue
        .iter()
        .map(|line| {
            format!(
                "- **{}:** {}  \n  _{}_",
                line.speaker, line.target, line.translation
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let grammar = unit
        .grammar
        .iter()
        .map(|topic| format!("- {topic}"))
        .collect::<Vec<_>>()
        .join("\n");
    let pronunciation = unit
        .pronunciation
        .iter()
        .map(|note| format!("- {note}"))
        .collect::<Vec<_>>()
        .join("\n");
    let sources = curriculum(language)
        .map(|curriculum| {
            curriculum
                .official_sources
                .iter()
                .take(3)
                .map(|source| format!("- [{}]({})", source.title, source.url))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let markdown = format!(
        "## Mission\n\n**Scenario:** {scenario}\n\n**CEFR {level} can-do:** {can_do}\n\n\
         This is a **{phase_label}** pass through the scenario. {phase_guidance}\n\n\
         {foundations}\
         ## Model dialogue\n\n{dialogue}\n\n\
         ### How to work with it\n\n1. Listen once without translating every word.\n2. Shadow each target-language line at natural speed.\n3. Replace names, places, times, or opinions with your own details.\n4. Close the transcript and reconstruct the exchange from memory.\n\n\
         ## Useful language chunks\n\n{phrases}\n\n\
         These are chunks, not isolated dictionary entries. Say the whole phrase aloud and notice what normally comes before and after it.\n\n\
         ## Grammar in service of the task\n\n{grammar}\n\n\
         Do not memorise a rule without an example. Find one example in the dialogue, make one true example about yourself, and make one question that another person could answer.\n\n\
         ## Vocabulary in context\n\n| Target language | Meaning | Example |\n| --- | --- | --- |\n{vocabulary}\n\n\
         ## Pronunciation and listening\n\n{pronunciation}\n\n\
         Listen for stressed syllables and phrase rhythm, not only individual sounds. Comprehensibility matters before accent perfection.\n\n\
         ## Pragmatics\n\n{pragmatics}\n\n\
         ## Culture in context\n\n{culture}\n\n\
         ## Produce evidence\n\n**Speaking:** {speaking_prompt}\n\n**Writing:** {writing_prompt}\n\n\
         ## Why this progression is credible\n\nThe app revisits this scenario through reception, interaction, production, and delayed retrieval. Progress is based on demonstrated skill evidence, not merely opening the lesson. CEFR dates are planning targets rather than certificates; live conversation and varied real-world input remain necessary.\n\n\
         ## Reference framework\n\n{sources}",
        scenario = unit.scenario,
        can_do = unit.can_do,
        phase_label = phase_label(phase),
        phase_guidance = phase_guidance(phase),
        foundations = foundations,
        dialogue = dialogue,
        phrases = phrases,
        grammar = grammar,
        vocabulary = vocabulary,
        pronunciation = pronunciation,
        pragmatics = unit.pragmatics,
        culture = unit.culture,
        speaking_prompt = unit.speaking_prompt,
        writing_prompt = unit.writing_prompt,
        sources = sources,
    );
    let listen_text = unit
        .dialogue
        .iter()
        .map(|line| line.target.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    StoredLesson {
        title: unit.title.clone(),
        scenario: unit.scenario.clone(),
        can_do: unit.can_do.clone(),
        phase_label: phase_label(phase).to_string(),
        markdown,
        phrases: unit.phrases.clone(),
        dialogue: unit.dialogue.clone(),
        questions: build_questions(unit, phase),
        speaking_prompt: unit.speaking_prompt.clone(),
        writing_prompt: unit.writing_prompt.clone(),
        listen_text,
    }
}

fn select_unit(
    conn: &Connection,
    language: &str,
    level: &LevelSpec,
    revisit: bool,
) -> Result<(UnitSpec, i64)> {
    let mut progress = HashMap::new();
    let mut stmt = conn
        .prepare(
            "SELECT unit_slug, phase_completed, score_ema
             FROM language_unit_progress WHERE language = ?1",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([language], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, f64>(2)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    for row in rows {
        let (slug, phase, score) = row.map_err(|error| error.to_string())?;
        progress.insert(slug, (phase, score));
    }
    let max_phase = level.sessions_per_unit as i64;
    // Units carrying explicit foundations are prerequisites for later
    // scenarios. Complete their staged passes in seed order before the normal
    // breadth-first rotation begins.
    if let Some(unit) = level.units.iter().find(|unit| {
        !unit.foundations.is_empty()
            && progress
                .get(&unit.slug)
                .map(|entry| entry.0)
                .unwrap_or(0)
                .min(max_phase)
                < max_phase
    }) {
        let current = progress.get(&unit.slug).map(|entry| entry.0).unwrap_or(0);
        return Ok((unit.clone(), current + 1));
    }
    // Breadth-first rotation over *incomplete* units only: completed units
    // are never re-served automatically.
    let minimum_phase = level
        .units
        .iter()
        .map(|unit| {
            progress
                .get(&unit.slug)
                .map(|entry| entry.0)
                .unwrap_or(0)
                .min(max_phase)
        })
        .min()
        .unwrap_or(0);
    if minimum_phase < max_phase {
        if let Some(unit) = level.units.iter().find(|unit| {
            progress
                .get(&unit.slug)
                .map(|entry| entry.0)
                .unwrap_or(0)
                .min(max_phase)
                == minimum_phase
        }) {
            let current = progress.get(&unit.slug).map(|entry| entry.0).unwrap_or(0);
            return Ok((unit.clone(), current + 1));
        }
    }
    // Every unit in this level is complete. Only two paths remain: an
    // explicit revisit, or targeted remediation while the level's skill gate
    // is still unmet. A fully gated top level ends the program.
    if revisit || next_level(&level.level).is_some() {
        let weakest = level
            .units
            .iter()
            .min_by(|a, b| {
                let a_score = progress.get(&a.slug).map(|entry| entry.1).unwrap_or(0.0);
                let b_score = progress.get(&b.slug).map(|entry| entry.1).unwrap_or(0.0);
                a_score.total_cmp(&b_score)
            })
            .ok_or_else(|| format!("{} has no units", level.level))?;
        return Ok((weakest.clone(), max_phase + 1));
    }
    Err(format!(
        "{} is complete — every {} unit is finished. Revisit to practice again.",
        curriculum(language)?.label,
        level.level
    ))
}

struct LessonMeta<'a> {
    session_id: i64,
    language: &'a str,
    level: &'a str,
    unit_slug: &'a str,
    phase: i64,
    status: &'a str,
    estimated_minutes: i64,
}

fn lesson_view(meta: LessonMeta<'_>, stored: StoredLesson) -> Result<LanguageLessonView> {
    let curriculum = curriculum(meta.language)?;
    Ok(LanguageLessonView {
        session_id: meta.session_id,
        language: meta.language.to_string(),
        label: curriculum.label.clone(),
        native_label: curriculum.native_label.clone(),
        level: meta.level.to_string(),
        unit_slug: meta.unit_slug.to_string(),
        phase: meta.phase,
        phase_label: stored.phase_label,
        title: stored.title,
        scenario: stored.scenario,
        can_do: stored.can_do,
        markdown: stored.markdown,
        phrases: stored.phrases,
        dialogue: stored.dialogue,
        questions: stored
            .questions
            .into_iter()
            .map(|question| LanguageQuestionView {
                id: question.id,
                prompt: question.prompt,
                choices: question.choices,
                strand: question.strand,
            })
            .collect(),
        speaking_prompt: stored.speaking_prompt,
        writing_prompt: stored.writing_prompt,
        listen_text: stored.listen_text,
        estimated_minutes: meta.estimated_minutes,
        status: meta.status.to_string(),
    })
}

fn read_session(
    conn: &Connection,
    where_clause: &str,
    value: &dyn rusqlite::ToSql,
) -> Result<Option<LanguageLessonView>> {
    let sql = format!(
        "SELECT s.id, s.language, s.level, s.unit_slug, s.phase, s.status, s.lesson_json,
                p.session_minutes
         FROM language_sessions s
         JOIN language_programs p ON p.language = s.language
         WHERE {where_clause}
         ORDER BY s.id DESC LIMIT 1"
    );
    let row = conn
        .query_row(&sql, [value], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })
        .optional()
        .map_err(|error| error.to_string())?;
    row.map(
        |(id, language, level, slug, phase, status, lesson_json, minutes)| {
            let stored: StoredLesson =
                serde_json::from_str(&lesson_json).map_err(|error| error.to_string())?;
            lesson_view(
                LessonMeta {
                    session_id: id,
                    language: &language,
                    level: &level,
                    unit_slug: &slug,
                    phase,
                    status: &status,
                    estimated_minutes: minutes,
                },
                stored,
            )
        },
    )
    .transpose()
}

pub fn active_session(conn: &Connection) -> Result<Option<LanguageLessonView>> {
    read_session(conn, "s.status = 'in_progress' AND ?1 = 1", &1_i64)
}

pub fn active_session_for(conn: &Connection, language: &str) -> Result<Option<LanguageLessonView>> {
    read_session(
        conn,
        "s.status = 'in_progress' AND s.language = ?1",
        &language,
    )
}

#[derive(Debug, Clone, Serialize)]
pub struct ActiveLanguageSessionView {
    pub session_id: i64,
    pub language: String,
    pub label: String,
    pub level: String,
    pub title: String,
}

pub fn active_summary(conn: &Connection) -> Result<Option<ActiveLanguageSessionView>> {
    Ok(
        active_session(conn)?.map(|session| ActiveLanguageSessionView {
            session_id: session.session_id,
            language: session.language,
            label: session.label,
            level: session.level,
            title: session.title,
        }),
    )
}

pub fn active_summaries(conn: &Connection) -> Result<Vec<ActiveLanguageSessionView>> {
    LANGUAGES
        .iter()
        .map(|language| active_session_for(conn, language))
        .filter_map(|result| match result {
            Ok(Some(session)) => Some(Ok(ActiveLanguageSessionView {
                session_id: session.session_id,
                language: session.language,
                label: session.label,
                level: session.level,
                title: session.title,
            })),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        })
        .collect()
}

pub fn start_session(
    conn: &Connection,
    language: &str,
    slot_id: Option<i64>,
    today: &str,
    revisit: bool,
) -> Result<LanguageLessonView> {
    if !valid_language(language) {
        return Err(format!("unsupported language: {language}"));
    }
    let program = program_row(conn, language)?;
    if !program.enabled {
        return Err(format!("{language} is not enabled"));
    }
    if let Some(active) = active_session_for(conn, language)? {
        return Ok(active);
    }
    if let Some(id) = slot_id {
        let slot_language: Option<String> = conn
            .query_row(
                "SELECT language FROM language_schedule_slots WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if slot_language.as_deref() != Some(language) {
            return Err("schedule slot does not belong to this language".into());
        }
        let consumed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM language_sessions
                 WHERE slot_id = ?1 AND session_date = ?2",
                params![id, today],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if consumed > 0 {
            return Err("this language slot is already complete for today".into());
        }
    }
    let curriculum = curriculum(language)?;
    let level = level_spec(curriculum, &program.current_level)?;
    let (unit, phase) = select_unit(conn, language, level, revisit)?;
    let stored = build_lesson(language, &program.current_level, &unit, phase);
    let lesson_json = serde_json::to_string(&stored).map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO language_sessions
            (slot_id, language, session_date, level, unit_slug, phase, status,
             lesson_json, started_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'in_progress', ?7, ?8)",
        params![
            slot_id,
            language,
            today,
            program.current_level,
            unit.slug,
            phase,
            lesson_json,
            now_iso()
        ],
    )
    .map_err(|error| error.to_string())?;
    lesson_view(
        LessonMeta {
            session_id: conn.last_insert_rowid(),
            language,
            level: &program.current_level,
            unit_slug: &unit.slug,
            phase,
            status: "in_progress",
            estimated_minutes: program.session_minutes,
        },
        stored,
    )
}

/// Start the language engine from a generic classroom slot. The curriculum,
/// CEFR evidence, and session payload remain language-owned; only scheduling
/// is generalized. `revisit` is the opt-in path for re-practicing a completed
/// unit.
pub fn start_classroom_session(
    conn: &Connection,
    language: &str,
    classroom_slot_id: Option<i64>,
    today: &str,
    revisit: bool,
) -> Result<LanguageLessonView> {
    if let Some(id) = classroom_slot_id {
        let owner: Option<(String, String)> = conn
            .query_row(
                "SELECT s.subject_id, p.kind
                 FROM classroom_schedule_slots s
                 JOIN classroom_programs p ON p.subject_id = s.subject_id
                 WHERE s.id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if owner.as_ref().map(|row| row.0.as_str()) != Some(language)
            || owner.as_ref().map(|row| row.1.as_str()) != Some("language")
        {
            return Err("classroom slot does not belong to this language".into());
        }
        let consumed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM language_sessions
                 WHERE classroom_slot_id = ?1 AND session_date = ?2",
                params![id, today],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if consumed > 0 {
            return Err("this class slot is already complete for today".into());
        }
    }
    let lesson = start_session(conn, language, None, today, revisit)?;
    if let Some(id) = classroom_slot_id {
        conn.execute(
            "UPDATE language_sessions
             SET classroom_slot_id = COALESCE(classroom_slot_id, ?2)
             WHERE id = ?1",
            params![lesson.session_id, id],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(lesson)
}

pub(crate) fn stored_lesson(conn: &Connection, session_id: i64) -> Result<StoredLesson> {
    let json = conn
        .query_row(
            "SELECT lesson_json FROM language_sessions WHERE id = ?1",
            [session_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&json).map_err(|error| error.to_string())
}

pub(crate) fn validate_generated_lesson(
    seed: &StoredLesson,
    generated: &StoredLesson,
) -> Result<()> {
    if generated.scenario.trim() != seed.scenario.trim()
        || generated.can_do.trim() != seed.can_do.trim()
        || generated.phase_label.trim() != seed.phase_label.trim()
    {
        return Err("generated language lesson changed the curated objective".into());
    }
    if generated.markdown.split_whitespace().count() < 450 {
        return Err("generated language lesson is too shallow".into());
    }
    if seed.markdown.contains("## First principles foundation")
        && !generated
            .markdown
            .contains("## First principles foundation")
    {
        return Err("generated language lesson removed its first-principles foundation".into());
    }
    if generated.phrases.len() < 6 || generated.dialogue.len() < 6 {
        return Err("generated language lesson lacks phrase or dialogue evidence".into());
    }
    if generated
        .phrases
        .iter()
        .any(|phrase| phrase.target.trim().is_empty() || phrase.translation.trim().is_empty())
        || generated.dialogue.iter().any(|line| {
            line.speaker.trim().is_empty()
                || line.target.trim().is_empty()
                || line.translation.trim().is_empty()
        })
    {
        return Err("generated language lesson contains empty teaching content".into());
    }
    if generated.speaking_prompt.split_whitespace().count() < 8
        || generated.writing_prompt.split_whitespace().count() < 8
        || generated.listen_text.split_whitespace().count() < 12
    {
        return Err("generated language production tasks are not actionable".into());
    }
    Ok(())
}

pub(crate) fn replace_stored_lesson(
    conn: &Connection,
    session_id: i64,
    stored: &StoredLesson,
) -> Result<LanguageLessonView> {
    let json = serde_json::to_string(stored).map_err(|error| error.to_string())?;
    conn.execute(
        "UPDATE language_sessions SET lesson_json = ?2
         WHERE id = ?1 AND status = 'in_progress'",
        params![session_id, json],
    )
    .map_err(|error| error.to_string())?;
    read_session(conn, "s.id = ?1", &session_id)?
        .ok_or_else(|| "active language lesson could not be reloaded".into())
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubmitSessionInput {
    pub session_id: i64,
    pub answers: Vec<usize>,
    pub writing_response: String,
    pub speaking_completed: bool,
    pub listened: bool,
    pub confidence: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct CorrectionView {
    pub question_id: usize,
    pub prompt: String,
    pub selected_answer: String,
    pub correct_answer: String,
    pub correct: bool,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LanguageSessionResult {
    pub session_id: i64,
    pub passed: bool,
    pub score: f64,
    pub corrections: Vec<CorrectionView>,
    pub level_advanced_to: Option<String>,
    pub current_level: String,
    pub progress: LanguageProgramView,
}

fn update_skill(
    conn: &Connection,
    language: &str,
    strand: &str,
    observation: f64,
    today: &str,
) -> Result<()> {
    if !STRANDS.contains(&strand) {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO language_skill_scores
            (language, strand, score_ema, encounters, last_seen_date)
         VALUES (?1, ?2, ?3, 1, ?4)
         ON CONFLICT(language, strand) DO UPDATE SET
            score_ema = CASE
                WHEN language_skill_scores.encounters = 0 THEN excluded.score_ema
                ELSE 0.65 * language_skill_scores.score_ema + 0.35 * excluded.score_ema
            END,
            encounters = language_skill_scores.encounters + 1,
            last_seen_date = excluded.last_seen_date",
        params![language, strand, observation.clamp(0.0, 1.0), today],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn next_level(level: &str) -> Option<&'static str> {
    match level {
        "A1" => Some("A2"),
        "A2" => Some("B1"),
        "B1" => Some("B2"),
        _ => None,
    }
}

fn maybe_advance_level(
    conn: &Connection,
    language: &str,
    current_level: &str,
) -> Result<Option<String>> {
    let curriculum = curriculum(language)?;
    let level = level_spec(curriculum, current_level)?;
    let (completed, required) = level_progress(conn, language, level)?;
    if completed < required {
        return Ok(None);
    }
    let skills = skills_for(conn, language)?;
    let evidence_ready = skills
        .iter()
        .all(|skill| skill.encounters > 0 && skill.score >= 0.60);
    if !evidence_ready {
        return Ok(None);
    }
    let Some(next) = next_level(current_level) else {
        return Ok(None);
    };
    conn.execute(
        "UPDATE language_programs SET current_level = ?2, updated_at = ?3
         WHERE language = ?1",
        params![language, next, now_iso()],
    )
    .map_err(|error| error.to_string())?;
    Ok(Some(next.to_string()))
}

pub fn submit_session(
    conn: &Connection,
    input: &SubmitSessionInput,
    today: &str,
) -> Result<LanguageSessionResult> {
    let row = conn
        .query_row(
            "SELECT language, level, unit_slug, phase, status, lesson_json
             FROM language_sessions WHERE id = ?1",
            [input.session_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .map_err(|error| error.to_string())?;
    let (language, level, unit_slug, phase, status, lesson_json) = row;
    if status != "in_progress" {
        return Err("this language session is already finished".into());
    }
    let lesson: StoredLesson =
        serde_json::from_str(&lesson_json).map_err(|error| error.to_string())?;
    if input.answers.len() != lesson.questions.len() {
        return Err("answer every knowledge check before submitting".into());
    }
    let mut correct_count = 0;
    let mut corrections = Vec::new();
    for (index, question) in lesson.questions.iter().enumerate() {
        let selected = input.answers[index];
        if selected >= question.choices.len() {
            return Err(format!("answer {} is invalid", index + 1));
        }
        let correct = selected == question.correct_index;
        if correct {
            correct_count += 1;
        }
        update_skill(
            conn,
            &language,
            &question.strand,
            if correct { 1.0 } else { 0.0 },
            today,
        )?;
        corrections.push(CorrectionView {
            question_id: question.id,
            prompt: question.prompt.clone(),
            selected_answer: question.choices[selected].clone(),
            correct_answer: question.choices[question.correct_index].clone(),
            correct,
            explanation: question.explanation.clone(),
        });
    }
    let knowledge_score = correct_count as f64 / lesson.questions.len() as f64;
    let writing_words = input.writing_response.split_whitespace().count();
    let writing_evidence = if writing_words >= 12 {
        (input.confidence.clamp(1, 5) as f64 / 5.0).max(0.6)
    } else if writing_words > 0 {
        0.35
    } else {
        0.0
    };
    update_skill(conn, &language, "writing", writing_evidence, today)?;
    if input.speaking_completed {
        let spoken = (input.confidence.clamp(1, 5) as f64 / 5.0).max(0.6);
        update_skill(conn, &language, "spoken_production", spoken, today)?;
        update_skill(conn, &language, "spoken_interaction", spoken * 0.9, today)?;
    }
    if input.listened {
        update_skill(conn, &language, "listening", knowledge_score, today)?;
    }
    let production_score = (writing_evidence + f64::from(input.speaking_completed)) / 2.0;
    let score = (knowledge_score * 0.8 + production_score * 0.2).clamp(0.0, 1.0);
    let passed = knowledge_score >= 0.60;
    let max_phase = level_spec(curriculum(&language)?, &level)?.sessions_per_unit as i64;
    let next_review = parse_date(today)
        + Duration::days(if score >= 0.85 {
            7
        } else if passed {
            3
        } else {
            1
        });
    conn.execute(
        "INSERT INTO language_unit_progress
            (language, unit_slug, phase_completed, score_ema, encounters,
             last_seen_date, next_review_date)
         VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)
         ON CONFLICT(language, unit_slug) DO UPDATE SET
            phase_completed = MAX(
                language_unit_progress.phase_completed,
                excluded.phase_completed
            ),
            score_ema = CASE
                WHEN language_unit_progress.encounters = 0 THEN excluded.score_ema
                ELSE 0.65 * language_unit_progress.score_ema + 0.35 * excluded.score_ema
            END,
            encounters = language_unit_progress.encounters + 1,
            last_seen_date = excluded.last_seen_date,
            next_review_date = excluded.next_review_date",
        params![
            language,
            unit_slug,
            if passed { phase.min(max_phase) } else { 0 },
            score,
            today,
            format_date(next_review)
        ],
    )
    .map_err(|error| error.to_string())?;
    let response_json = serde_json::json!({
        "answers": input.answers,
        "writing_response": input.writing_response,
        "speaking_completed": input.speaking_completed,
        "listened": input.listened,
        "confidence": input.confidence,
    })
    .to_string();
    conn.execute(
        "UPDATE language_sessions
         SET status = 'completed', score = ?2, response_json = ?3, completed_at = ?4
         WHERE id = ?1",
        params![input.session_id, score, response_json, now_iso()],
    )
    .map_err(|error| error.to_string())?;
    let level_advanced_to = if passed {
        maybe_advance_level(conn, &language, &level)?
    } else {
        None
    };
    let current_level = program_row(conn, &language)?.current_level;
    Ok(LanguageSessionResult {
        session_id: input.session_id,
        passed,
        score,
        corrections,
        level_advanced_to,
        current_level,
        progress: program_view(conn, &language, today)?,
    })
}

pub fn now_iso() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

pub fn language_name(language: &str) -> Result<String> {
    Ok(curriculum(language)?.label.clone())
}

pub fn speech_locale(language: &str) -> Result<&'static str> {
    match language {
        "german" => Ok("de-DE"),
        "italian" => Ok("it-IT"),
        _ => Err(format!("unsupported language: {language}")),
    }
}

pub fn pedagogical_summary(language: &str) -> Result<String> {
    let curriculum = curriculum(language)?;
    Ok(format!(
        "{} progresses through {} action-oriented CEFR units from A1 to B2. A1 starts from first principles with the alphabet, sound-spelling, and counting before later scenarios. Each scenario interleaves reception, interaction, production, grammar, vocabulary, pronunciation, and pragmatics.",
        target_language_name(language),
        curriculum.levels.iter().map(|level| level.units.len()).sum::<usize>()
    ))
}

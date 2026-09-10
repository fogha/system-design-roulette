//! Read-only input to the primary session cutover. Capture the original SQLite
//! values in one snapshot before constructing shared sessions/lesson versions.
//! This deliberately does not infer class ownership, tutor identity or learning
//! credit from today's configuration or from a pre-drawn concept.
use crate::db::{DbError, Result};
use rusqlite::{types::ValueRef, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// Preserve SQLite storage types and bytes, including unknown JSON formats.
/// Floating point bits avoid turning a non-finite SQLite real into JSON null.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Cell {
    Null,
    Integer(i64),
    RealBits(u64),
    Text(Vec<u8>),
    Blob(Vec<u8>),
}
pub type StoredRow = BTreeMap<String, Cell>;
pub type Tables = BTreeMap<String, Vec<StoredRow>>;

fn text<'a>(row: &'a StoredRow, column: &str) -> Option<&'a str> {
    match row.get(column) {
        Some(Cell::Text(bytes)) => std::str::from_utf8(bytes).ok(),
        _ => None,
    }
}
fn integer(row: &StoredRow, column: &str) -> Option<i64> {
    match row.get(column) {
        Some(Cell::Integer(value)) => Some(*value),
        _ => None,
    }
}
fn invalid(message: impl Into<String>) -> DbError {
    DbError::Invalid(message.into())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContentSelection {
    None,
    Stored {
        course_id: i64,
    },
    /// Keep every candidate. Choosing the last generated row could silently
    /// replace a previously displayed document, its exit check and its drafts.
    Ambiguous {
        course_ids: Vec<i64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum SubjectResolution {
    NotSelected,
    Recorded { course_id: String },
    LinkedConcept { course_id: String, concept_id: i64 },
    Unknown,
}
impl SubjectResolution {
    fn course_id(&self) -> Option<&str> {
        match self {
            Self::Recorded { course_id } | Self::LinkedConcept { course_id, .. } => Some(course_id),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryReason {
    InvalidServiceDate,
    UnknownSessionStatus,
    UnknownStage,
    UnknownSessionKind,
    InvalidReadingTime,
    SubjectNotRecorded,
    ConceptMissing,
    SubjectConceptMismatch,
    AmbiguousLesson,
    ReadingWithoutLesson,
    TerminalTimeNotRecorded,
    LessonMetadataUnknown,
    AssessmentFormatUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrimaryRecord {
    /// Reuse v7 identity; never generate a replacement during import.
    pub session_id: String,
    pub service_date: String,
    /// All primary records remain unassigned to a class. Subject recovery from
    /// a linked concept is explicit and never applied to an unstarted day.
    pub subject: SubjectResolution,
    pub original_status: String,
    pub original_step: String,
    pub content: ContentSelection,
    pub recovery: Vec<RecoveryReason>,
    pub assessment_attempt_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrimaryImport {
    pub format_version: u32,
    pub source_schema_version: u32,
    pub fingerprint: String,
    pub sessions: Vec<PrimaryRecord>,
    /// Documents without a daily row still belong in the archive. Do not attach
    /// them to a different day or discard their questions and exercise drafts.
    pub unattached_course_ids: Vec<i64>,
    /// Canonical source material; strings containing JSON remain original bytes.
    pub tables: Tables,
}

// Read only primary-owned learning records. In particular do not export runner
// credentials, arbitrary config/profile values, or another class's exercise.
const SOURCES: &[(&str, &str)] = &[
    ("sessions", "SELECT * FROM sessions ORDER BY date"),
    ("primary_session_ids", "SELECT * FROM primary_session_ids ORDER BY legacy_date"),
    ("legacy_crosswalk", "SELECT * FROM legacy_crosswalk WHERE legacy_table='sessions' ORDER BY legacy_key"),
    ("concepts", "SELECT * FROM concepts WHERE id IN (SELECT concept_id FROM sessions UNION SELECT concept_id FROM courses) ORDER BY id"),
    ("courses", "SELECT * FROM courses ORDER BY id"),
    ("questions", "SELECT * FROM questions ORDER BY id"),
    ("attempts", "SELECT * FROM attempts ORDER BY id"),
    ("carryover", "SELECT * FROM carryover ORDER BY question_id"),
    ("exit_questions", "SELECT * FROM exit_questions ORDER BY id"),
    ("exit_attempts", "SELECT * FROM exit_attempts ORDER BY id"),
    ("course_exercises", "SELECT * FROM course_exercises ORDER BY course_id"),
    ("exercise_drafts", "SELECT * FROM exercise_drafts WHERE course_id IS NOT NULL ORDER BY id"),
    ("audio_scripts", "SELECT * FROM audio_scripts ORDER BY course_id"),
    ("generation_jobs", "SELECT * FROM generation_jobs ORDER BY id"),
    ("assessment_attempts", "SELECT * FROM assessment_attempts WHERE owner_kind='legacy_primary' ORDER BY id"),
    ("assessment_rounds", "SELECT r.* FROM assessment_rounds r JOIN assessment_attempts a ON a.id=r.attempt_id WHERE a.owner_kind='legacy_primary' ORDER BY r.id"),
    ("assessment_work", "SELECT w.* FROM assessment_work w JOIN assessment_rounds r ON r.id=w.round_id JOIN assessment_attempts a ON a.id=r.attempt_id WHERE a.owner_kind='legacy_primary' ORDER BY w.round_id"),
    ("assessment_submissions", "SELECT s.* FROM assessment_submissions s JOIN assessment_rounds r ON r.id=s.round_id JOIN assessment_attempts a ON a.id=r.attempt_id WHERE a.owner_kind='legacy_primary' ORDER BY s.round_id"),
    ("legacy_assessment_config", "SELECT * FROM legacy_assessment_config ORDER BY key"),
    ("primary_config", "SELECT * FROM config WHERE key GLOB 'planned:*' OR key GLOB 'pop_quiz_set:*' OR key GLOB 'exit_quiz_round:*' OR key GLOB 'exit_quiz_count:*' OR key GLOB 'exit_failed_areas:*' ORDER BY key"),
];

fn read_rows(conn: &Connection, sql: &str) -> Result<Vec<StoredRow>> {
    let mut statement = conn.prepare(sql)?;
    let columns: Vec<String> = statement
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let rows = statement.query_map([], |row| {
        columns
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let cell = match row.get_ref(index)? {
                    ValueRef::Null => Cell::Null,
                    ValueRef::Integer(value) => Cell::Integer(value),
                    ValueRef::Real(value) => Cell::RealBits(value.to_bits()),
                    ValueRef::Text(bytes) => Cell::Text(bytes.to_vec()),
                    ValueRef::Blob(bytes) => Cell::Blob(bytes.to_vec()),
                };
                Ok((name.clone(), cell))
            })
            .collect::<rusqlite::Result<StoredRow>>()
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn fingerprint(tables: &Tables) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(tables)?)))
}

fn assessment_needs_recovery(tables: &Tables, attempt_ids: &[String]) -> bool {
    use crate::domain::assessments::{Item, Response};
    tables["assessment_rounds"]
        .iter()
        .filter(|row| {
            text(row, "attempt_id")
                .is_some_and(|id| attempt_ids.iter().any(|expected| expected == id))
        })
        .any(|row| {
            let Some(round_id) = text(row, "id") else {
                return true;
            };
            let Some(items) =
                text(row, "items_json").and_then(|raw| serde_json::from_str::<Vec<Item>>(raw).ok())
            else {
                return true;
            };
            let ids: BTreeSet<_> = items.iter().map(|item| item.id.as_str()).collect();
            if ids.len() != items.len()
                || items.iter().any(|item| {
                    serde_json::from_value::<crate::db::Question>(item.body.clone()).map_or(
                        true,
                        |question| {
                            question.id.to_string() != item.id
                                || !matches!(question.kind.as_str(), "mcq" | "free")
                        },
                    )
                })
            {
                return true;
            }
            let work: Vec<_> = tables["assessment_work"]
                .iter()
                .filter(|work| text(work, "round_id") == Some(round_id))
                .collect();
            if work.len() != 1 {
                return true;
            }
            let responses = text(work[0], "responses_json")
                .and_then(|raw| serde_json::from_str::<BTreeMap<String, Response>>(raw).ok());
            responses.is_none_or(|responses| responses.keys().any(|id| !ids.contains(id.as_str())))
        })
        || attempt_ids.iter().any(|id| {
            !tables["assessment_rounds"]
                .iter()
                .any(|round| text(round, "attempt_id") == Some(id.as_str()))
        })
}

/// No writes, provider calls, seeding, scheduling, or default-setting reads.
/// When called inside the migration's write transaction, reuse that snapshot.
pub fn inspect(conn: &Connection) -> Result<PrimaryImport> {
    if conn.is_autocommit() {
        let tx = conn.unchecked_transaction()?;
        let result = inspect(&tx)?;
        tx.commit()?;
        return Ok(result);
    }
    let version: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    // v8 only rebuilt classroom check evidence; the primary tables are the v7 shape.
    if !(7..=8).contains(&version) {
        return Err(invalid(format!(
            "Primary import requires schema v7 or v8, found v{version}."
        )));
    }
    super::migrations::validate_recorded_schema(conn)?;
    let tables: Tables = SOURCES
        .iter()
        .map(|(name, sql)| Ok((name.to_string(), read_rows(conn, sql)?)))
        .collect::<Result<_>>()?;
    let mut sessions = Vec::new();
    let mut used_ids = BTreeSet::new();
    for row in &tables["sessions"] {
        let date = text(row, "date").ok_or_else(|| invalid("A primary session has a non-text service date; original bytes must be repaired before migration."))?;
        let identities: Vec<_> = tables["primary_session_ids"]
            .iter()
            .filter(|id| text(id, "legacy_date") == Some(date))
            .collect();
        let [identity] = identities.as_slice() else {
            return Err(invalid(format!(
                "Session {date} must have exactly one saved primary identity."
            )));
        };
        let id = text(identity, "session_id")
            .filter(|id| !id.trim().is_empty())
            .ok_or_else(|| invalid("A saved primary identity is empty or malformed."))?;
        if !used_ids.insert(id.to_string()) {
            return Err(invalid("Two primary rows share a saved identity."));
        }
        let crosswalks: Vec<_> = tables["legacy_crosswalk"]
            .iter()
            .filter(|entry| text(entry, "legacy_key") == Some(date))
            .collect();
        if crosswalks.len() != 1
            || text(crosswalks[0], "entity_id") != Some(id)
            || text(crosswalks[0], "entity_kind") != Some("primary_session")
        {
            return Err(invalid(format!(
                "Session {date} has an inconsistent identity crosswalk."
            )));
        }
        let status = text(row, "status").unwrap_or_default();
        let step = text(row, "current_step").unwrap_or_default();
        let recorded_subject = text(row, "focus").filter(|s| !s.is_empty());
        let concept_id = integer(row, "concept_id");
        let concept = concept_id.and_then(|id| {
            tables["concepts"]
                .iter()
                .find(|c| integer(c, "id") == Some(id))
        });
        let subject = if let Some(course_id) = recorded_subject {
            SubjectResolution::Recorded {
                course_id: course_id.into(),
            }
        } else if status == "pending" {
            SubjectResolution::NotSelected
        } else if matches!(status, "in_progress" | "completed")
            || text(row, "started_at").is_some_and(|time| !time.is_empty())
        {
            match concept.and_then(|c| {
                Some((
                    integer(c, "id")?,
                    text(c, "focus").filter(|s| !s.is_empty())?,
                ))
            }) {
                Some((concept_id, course_id)) => SubjectResolution::LinkedConcept {
                    course_id: course_id.into(),
                    concept_id,
                },
                None => SubjectResolution::Unknown,
            }
        } else {
            SubjectResolution::Unknown
        };
        let candidates: Vec<_> = tables["courses"]
            .iter()
            .filter(|c| text(c, "session_date") == Some(date))
            .collect();
        let matching: Vec<i64> = candidates
            .iter()
            .filter(|c| concept_id.is_some() && integer(c, "concept_id") == concept_id)
            .filter_map(|c| integer(c, "id"))
            .collect();
        let mut recovery = Vec::new();
        if chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
            recovery.push(RecoveryReason::InvalidServiceDate);
        }
        if !matches!(status, "pending" | "in_progress" | "completed" | "skipped") {
            recovery.push(RecoveryReason::UnknownSessionStatus);
        }
        if !matches!(step, "quiz" | "review" | "roulette" | "course" | "done") {
            recovery.push(RecoveryReason::UnknownStage);
        }
        if !matches!(text(row, "session_type"), Some("lesson" | "pop_quiz")) {
            recovery.push(RecoveryReason::UnknownSessionKind);
        }
        if integer(row, "reading_seconds").is_none_or(|seconds| seconds < 0) {
            recovery.push(RecoveryReason::InvalidReadingTime);
        }
        if subject.course_id().is_none() {
            recovery.push(RecoveryReason::SubjectNotRecorded);
        }
        if concept_id.is_some() && concept.is_none() {
            recovery.push(RecoveryReason::ConceptMissing);
        }
        if let (Some(subject), Some(concept)) = (subject.course_id(), concept) {
            if text(concept, "focus") != Some(subject) {
                recovery.push(RecoveryReason::SubjectConceptMismatch);
            }
        }
        let content = if candidates.is_empty() {
            if status == "in_progress" && step == "course" {
                recovery.push(RecoveryReason::ReadingWithoutLesson);
            }
            ContentSelection::None
        } else if matching.len() == 1
            && subject.course_id().is_some()
            && !recovery.contains(&RecoveryReason::SubjectConceptMismatch)
        {
            ContentSelection::Stored {
                course_id: matching[0],
            }
        } else {
            recovery.push(RecoveryReason::AmbiguousLesson);
            ContentSelection::Ambiguous {
                course_ids: candidates.iter().filter_map(|c| integer(c, "id")).collect(),
            }
        };
        if let ContentSelection::Stored { course_id } = &content {
            let course = candidates
                .iter()
                .find(|c| integer(c, "id") == Some(*course_id))
                .unwrap();
            if text(course, "resources_json")
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
                .is_none_or(|value| !value.is_array())
            {
                recovery.push(RecoveryReason::LessonMetadataUnknown);
            }
        }
        if matches!(status, "completed" | "skipped")
            && text(row, "completed_at").is_none_or(|time| time.is_empty())
        {
            recovery.push(RecoveryReason::TerminalTimeNotRecorded);
        }
        let attempt_ids: Vec<String> = tables["assessment_attempts"]
            .iter()
            .filter(|a| text(a, "owner_key") == Some(date))
            .filter_map(|a| text(a, "id").map(str::to_string))
            .collect();
        let old_assessment_unknown = tables["legacy_assessment_config"]
            .iter()
            .filter(|r| text(r, "key").is_some_and(|key| key.ends_with(&format!(":{date}"))))
            .any(|r| {
                let Some(raw) = text(r, "value") else {
                    return true;
                };
                let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
                    return true;
                };
                match text(r, "key").and_then(|k| k.split(':').next()) {
                    Some("quiz_round") => {
                        serde_json::from_value::<Vec<crate::db::Question>>(value).is_err()
                    }
                    Some("pending_answers") => {
                        serde_json::from_value::<BTreeMap<String, String>>(value).is_err()
                    }
                    Some("quiz_result") => {
                        serde_json::from_value::<crate::commands::ReviewData>(value).is_err()
                    }
                    _ => true,
                }
            });
        if old_assessment_unknown || assessment_needs_recovery(&tables, &attempt_ids) {
            recovery.push(RecoveryReason::AssessmentFormatUnknown);
        }
        sessions.push(PrimaryRecord {
            session_id: id.to_string(),
            service_date: date.to_string(),
            subject,
            original_status: status.to_string(),
            original_step: step.to_string(),
            content,
            recovery,
            assessment_attempt_ids: attempt_ids,
        });
    }
    if used_ids.len() != tables["primary_session_ids"].len()
        || sessions.len() != tables["legacy_crosswalk"].len()
    {
        return Err(invalid(
            "A primary identity or crosswalk has no original session.",
        ));
    }
    let dates: BTreeSet<_> = sessions.iter().map(|s| s.service_date.as_str()).collect();
    let unattached_course_ids = tables["courses"]
        .iter()
        .filter(|c| text(c, "session_date").is_none_or(|date| !dates.contains(date)))
        .filter_map(|c| integer(c, "id"))
        .collect();
    Ok(PrimaryImport {
        format_version: 1,
        source_schema_version: version,
        fingerprint: fingerprint(&tables)?,
        sessions,
        unattached_course_ids,
        tables,
    })
}

/// Call again under the eventual cutover's write reservation before applying a
/// previously reviewed import. A lesson, timer tick, draft or worker publication
/// after inspection invalidates it; do not silently import stale work.
pub fn verify_unchanged(conn: &Connection, expected: &PrimaryImport) -> Result<()> {
    let current = inspect(conn)?;
    if current != *expected {
        return Err(invalid("Primary learning data changed after inspection. Build a fresh import before switching its writer."));
    }
    Ok(())
}

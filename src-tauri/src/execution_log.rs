//! The execution log: every line the tutor's runner reports, kept per run.
//!
//! The live feed used to vanish with the banner that showed it. A failed
//! preparation is only debuggable from what the runner said, so lines are
//! stored as they arrive, grouped by the session being prepared, and the desk
//! has a page to read them back.

use crate::db::Result;
use rusqlite::{params, Connection};
use serde::Serialize;

/// Lines older than this are pruned at startup.
const KEEP_DAYS: i64 = 30;
/// A single run keeps at most this many lines; older ones in the run go first.
const LINES_PER_RUN: i64 = 4_000;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LogLine {
    pub id: i64,
    pub at: String,
    pub run_id: Option<String>,
    pub course_id: Option<String>,
    pub line: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RunSummary {
    pub run_id: String,
    pub course_id: Option<String>,
    pub label: String,
    pub started_at: String,
    pub last_at: String,
    pub lines: i64,
    /// `running`, `ready`, `failed` or `unknown` from the preparation job, when
    /// the run was a session preparation.
    pub outcome: String,
    pub error: Option<String>,
}

pub fn append(
    conn: &Connection,
    run: Option<(&str, &str)>,
    line: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    let (run_id, course_id) = match run {
        Some((run, course)) => (Some(run), Some(course)),
        None => (None, None),
    };
    conn.execute(
        "INSERT INTO execution_log_lines (at, run_id, course_id, line) VALUES (?1, ?2, ?3, ?4)",
        params![now.to_rfc3339(), run_id, course_id, line],
    )?;
    if let Some(run) = run_id {
        conn.execute(
            "DELETE FROM execution_log_lines WHERE run_id = ?1 AND id NOT IN (
                SELECT id FROM execution_log_lines WHERE run_id = ?1 ORDER BY id DESC LIMIT ?2)",
            params![run, LINES_PER_RUN],
        )?;
    }
    Ok(())
}

/// Forget lines older than the retention window.
pub fn prune(conn: &Connection, now: chrono::DateTime<chrono::Utc>) -> Result<usize> {
    let cutoff = (now - chrono::Duration::days(KEEP_DAYS)).to_rfc3339();
    Ok(conn.execute("DELETE FROM execution_log_lines WHERE at < ?1", [cutoff])?)
}

/// Runs, newest first.
pub fn runs(conn: &Connection, limit: i64) -> Result<Vec<RunSummary>> {
    let mut statement = conn.prepare(
        "SELECT l.run_id, l.course_id, MIN(l.at), MAX(l.at), COUNT(*),
                COALESCE(j.status, 'unknown'), j.error,
                COALESCE(p.label, l.course_id, 'desk')
         FROM execution_log_lines l
         LEFT JOIN study_preparation_jobs j ON j.session_id = l.run_id
         LEFT JOIN classroom_programs p ON p.subject_id = l.course_id
         WHERE l.run_id IS NOT NULL
         GROUP BY l.run_id
         ORDER BY MAX(l.at) DESC
         LIMIT ?1",
    )?;
    let rows = statement
        .query_map([limit], |r| {
            Ok(RunSummary {
                run_id: r.get(0)?,
                course_id: r.get(1)?,
                started_at: r.get(2)?,
                last_at: r.get(3)?,
                lines: r.get(4)?,
                outcome: r.get(5)?,
                error: r.get(6)?,
                label: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Every line of one run, oldest first.
pub fn lines(conn: &Connection, run_id: &str) -> Result<Vec<LogLine>> {
    let mut statement = conn.prepare(
        "SELECT id, at, run_id, course_id, line FROM execution_log_lines WHERE run_id = ?1 ORDER BY id",
    )?;
    let rows = statement
        .query_map([run_id], |r| {
            Ok(LogLine {
                id: r.get(0)?,
                at: r.get(1)?,
                run_id: r.get(2)?,
                course_id: r.get(3)?,
                line: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// The most recent lines regardless of run, oldest first, for a live tail.
pub fn recent(conn: &Connection, limit: i64) -> Result<Vec<LogLine>> {
    let mut statement = conn.prepare(
        "SELECT id, at, run_id, course_id, line FROM (
            SELECT id, at, run_id, course_id, line FROM execution_log_lines ORDER BY id DESC LIMIT ?1
         ) ORDER BY id",
    )?;
    let rows = statement
        .query_map([limit], |r| {
            Ok(LogLine {
                id: r.get(0)?,
                at: r.get(1)?,
                run_id: r.get(2)?,
                course_id: r.get(3)?,
                line: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

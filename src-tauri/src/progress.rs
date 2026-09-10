//! Read-only progress across compatibility engines. Switch this projection with
//! the shared-runtime writers during their cutover; never union both copies.
use crate::classroom;
use chrono::{Duration, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

const PAGE_SIZE: i64 = 8;
const HISTORY: &str = r#"WITH history AS (
 SELECT 'primary' AS source, i.session_id AS owner_id, s.date,
 COALESCE(NULLIF(s.focus,''),c.focus,'legacy') AS subject_id,
 COALESCE(c.title,'Saved study session') AS title, s.status, s.quiz_score AS score,
 (SELECT id FROM courses WHERE session_date=s.date AND concept_id=s.concept_id ORDER BY id DESC LIMIT 1) AS course_id, s.started_at
 FROM sessions s JOIN primary_session_ids i ON i.legacy_date=s.date
 LEFT JOIN concepts c ON c.id=s.concept_id WHERE s.status!='pending'
 UNION ALL
 SELECT 'classroom',CAST(id AS TEXT),session_date,subject_id,title,status,score,NULL,started_at FROM classroom_sessions
 UNION ALL
 SELECT 'language',CAST(id AS TEXT),session_date,language,
 CASE WHEN json_valid(lesson_json) THEN COALESCE(json_extract(lesson_json,'$.title'),unit_slug) ELSE unit_slug END,
 status,score,NULL,started_at FROM language_sessions
)"#;

#[derive(Debug, Default, Deserialize)]
pub struct ProgressQuery {
    pub subject_id: Option<String>,
    #[serde(default)]
    pub search: String,
    pub status: Option<String>,
    #[serde(default)]
    pub page: i64,
}
#[derive(Debug, Serialize)]
pub struct ProgressEntry {
    pub source: String,
    pub owner_id: String,
    pub date: String,
    pub subject_id: String,
    pub title: String,
    pub status: String,
    pub score: Option<f64>,
    pub can_read: bool,
}
#[derive(Debug, Serialize)]
pub struct ProgressClass {
    pub subject_id: String,
    pub label: String,
    pub short_code: String,
    pub enabled: bool,
    pub progress: f64,
    pub progress_label: String,
    pub completed_sessions: i64,
}
#[derive(Debug, Serialize)]
pub struct ActivityDay {
    pub date: String,
    pub completed: i64,
}
#[derive(Debug, Serialize)]
pub struct DashboardView {
    pub today: String,
    pub classes: Vec<ProgressClass>,
    pub completed_sessions: i64,
    pub study_days: i64,
    pub streak: i64,
    pub activity: Vec<ActivityDay>,
    pub history: Vec<ProgressEntry>,
    pub history_total: i64,
    pub page: i64,
    pub page_size: i64,
}

pub fn read(
    conn: &Connection,
    today: &str,
    query: &ProgressQuery,
) -> Result<DashboardView, String> {
    if query
        .status
        .as_deref()
        .is_some_and(|s| !matches!(s, "completed" | "in_progress" | "skipped"))
    {
        return Err("Unknown progress status".into());
    }
    let today_date = NaiveDate::parse_from_str(today, "%Y-%m-%d").map_err(|e| e.to_string())?;
    let snapshot = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let conn = &*snapshot;
    let read = || -> rusqlite::Result<_> {
        let mut statement = conn.prepare(&format!("{HISTORY} SELECT date,COUNT(*) FROM history WHERE status='completed' AND date<=?1 AND (?2 IS NULL OR subject_id=?2) GROUP BY date ORDER BY date DESC"))?;
        let completed_dates = statement
            .query_map(params![today, query.subject_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let completed_sessions = completed_dates.iter().map(|(_, n)| n).sum();
        let study_days = completed_dates.len() as i64;
        let mut streak = 0;
        let mut expected = today_date;
        for (day, _) in &completed_dates {
            let Ok(day) = NaiveDate::parse_from_str(day, "%Y-%m-%d") else {
                continue;
            };
            if streak == 0 && day == today_date - Duration::days(1) {
                expected = day;
            }
            if day != expected {
                break;
            }
            streak += 1;
            expected -= Duration::days(1);
        }
        let activity = (0..28)
            .rev()
            .map(|ago| {
                let date = (today_date - Duration::days(ago)).to_string();
                let completed = completed_dates
                    .iter()
                    .find(|(d, _)| d == &date)
                    .map(|(_, n)| *n)
                    .unwrap_or(0);
                ActivityDay { date, completed }
            })
            .collect();
        let mut statement = conn.prepare(&format!("{HISTORY} SELECT subject_id,COUNT(*) FROM history WHERE status='completed' AND date<=?1 GROUP BY subject_id"))?;
        let counts = statement
            .query_map([today], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })?
            .collect::<rusqlite::Result<std::collections::HashMap<_, _>>>()?;
        let filter = "WHERE (?1 IS NULL OR subject_id=?1) AND (?2 IS NULL OR status=?2) AND (instr(lower(title),lower(?3))>0 OR instr(lower(subject_id),lower(?3))>0)";
        let search = query.search.trim();
        let history_total: i64 = conn.query_row(
            &format!("{HISTORY} SELECT COUNT(*) FROM history {filter}"),
            params![query.subject_id, query.status, search],
            |r| r.get(0),
        )?;
        let page = query
            .page
            .max(0)
            .min((history_total - 1).max(0) / PAGE_SIZE);
        let mut statement = conn.prepare(&format!("{HISTORY} SELECT source,owner_id,date,subject_id,title,status,score,source!='primary' OR course_id IS NOT NULL FROM history {filter} ORDER BY date DESC,started_at DESC,source,owner_id DESC LIMIT ?4 OFFSET ?5"))?;
        let history = statement
            .query_map(
                params![
                    query.subject_id,
                    query.status,
                    search,
                    PAGE_SIZE,
                    page * PAGE_SIZE
                ],
                |r| {
                    Ok(ProgressEntry {
                        source: r.get(0)?,
                        owner_id: r.get(1)?,
                        date: r.get(2)?,
                        subject_id: r.get(3)?,
                        title: r.get(4)?,
                        status: r.get(5)?,
                        score: r.get(6)?,
                        can_read: r.get(7)?,
                    })
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok((
            completed_sessions,
            study_days,
            streak,
            activity,
            counts,
            history_total,
            page,
            history,
        ))
    };
    let (completed_sessions, study_days, streak, activity, counts, history_total, page, history) =
        read().map_err(|e| e.to_string())?;
    let classes = classroom::program_views(conn, today)?
        .into_iter()
        .map(|p| ProgressClass {
            completed_sessions: counts.get(&p.subject_id).copied().unwrap_or(0),
            subject_id: p.subject_id,
            label: p.label,
            short_code: p.short_code,
            enabled: p.enabled,
            progress: p.progress,
            progress_label: p.progress_label,
        })
        .collect();
    snapshot.commit().map_err(|e| e.to_string())?;
    Ok(DashboardView {
        today: today.into(),
        classes,
        completed_sessions,
        study_days,
        streak,
        activity,
        history,
        history_total,
        page,
        page_size: PAGE_SIZE,
    })
}

#[derive(Debug, Serialize)]
pub struct ProgressLesson {
    pub title: String,
    pub date: String,
    pub markdown: String,
    pub course_id: Option<i64>,
    pub classroom_session_id: Option<i64>,
}

/// Resolve an immutable owner, never a date supplied by an archive row.
pub fn lesson(
    conn: &Connection,
    source: &str,
    owner_id: &str,
) -> Result<Option<ProgressLesson>, String> {
    if source == "primary" {
        return conn.query_row("SELECT c.id,s.date,COALESCE(k.title,'Saved study session'),c.markdown FROM primary_session_ids i JOIN sessions s ON s.date=i.legacy_date JOIN courses c ON c.session_date=s.date AND c.concept_id=s.concept_id LEFT JOIN concepts k ON k.id=c.concept_id WHERE i.session_id=?1 ORDER BY c.id DESC LIMIT 1",[owner_id],|r|Ok(ProgressLesson {course_id:Some(r.get(0)?),classroom_session_id:None,date:r.get(1)?,title:r.get(2)?,markdown:r.get(3)?})).optional().map_err(|e|e.to_string());
    }
    let sql = match source {
        "classroom" => "SELECT session_date,title,payload_json FROM classroom_sessions WHERE id=?1",
        "language" => {
            "SELECT session_date,unit_slug,lesson_json FROM language_sessions WHERE id=?1"
        }
        _ => return Err("Unknown lesson source".into()),
    };
    let id: i64 = owner_id.parse().map_err(|_| "Invalid lesson identity")?;
    let row = conn
        .query_row(sql, [id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .optional()
        .map_err(|e| e.to_string())?;
    row.map(|(date, title, payload)| {
        let body: serde_json::Value =
            serde_json::from_str(&payload).map_err(|_| "Saved lesson could not be read")?;
        let markdown = body["markdown"]
            .as_str()
            .ok_or("Saved lesson has no reading content")?
            .to_string();
        Ok(ProgressLesson {
            title: body["title"].as_str().unwrap_or(&title).into(),
            date,
            markdown,
            course_id: None,
            classroom_session_id: (source == "classroom").then_some(id),
        })
    })
    .transpose()
}

//! Accepted personal paths. Preference changes never manufacture learning credit.
//! The classroom adapters remain responsible for their existing session records.
use super::{
    enrollment::{self, DraftStatus, EnrollmentConfiguration, EnrollmentDraftId, LearningGoal},
    placement::{self, Recommendation},
};
use crate::{
    catalog,
    db::{self, DbError, Result},
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathReference {
    pub class_id: String,
    pub path_revision_id: String,
    pub course_snapshot_fingerprint: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptedPath {
    pub reference: PathReference,
    pub revision: u32,
    pub configuration: EnrollmentConfiguration,
    pub recommendation: Recommendation,
    pub accepted_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathSummary {
    pub class_id: String,
    pub path_revision_id: String,
    pub revision: u32,
    pub entry_point: String,
    pub entry_label: String,
    pub route: String,
    pub earlier_topics: usize,
    pub refreshers: usize,
}
impl AcceptedPath {
    pub fn summary(&self) -> PathSummary {
        PathSummary {
            class_id: self.reference.class_id.clone(),
            path_revision_id: self.reference.path_revision_id.clone(),
            revision: self.revision,
            entry_point: self.recommendation.entry_point.clone(),
            entry_label: self.recommendation.entry_label.clone(),
            route: self.recommendation.route.clone(),
            earlier_topics: self.recommendation.earlier_topics.len(),
            refreshers: self.recommendation.refreshers.len(),
        }
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptPath {
    pub draft_id: EnrollmentDraftId,
    pub expected_revision: u32,
    pub recommendation_id: String,
}
fn read_path(conn: &Connection, path_id: &str) -> Result<AcceptedPath> {
    let row = conn.query_row("SELECT id, class_id, revision, course_snapshot_fingerprint, entry_profile_json, plan_json, accepted_at FROM path_revisions WHERE id = ?1", [path_id], |r| {
        Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,u32>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?))
    })?;
    Ok(AcceptedPath {
        reference: PathReference {
            path_revision_id: row.0,
            class_id: row.1,
            course_snapshot_fingerprint: row.3,
        },
        revision: row.2,
        configuration: serde_json::from_str(&row.4)?,
        recommendation: serde_json::from_str(&row.5)?,
        accepted_at: row.6,
    })
}
pub fn current_path(conn: &Connection, course_id: &str) -> Result<Option<AcceptedPath>> {
    let id: Option<String> = conn
        .query_row(
            "SELECT active_path_revision_id FROM classes WHERE course_id = ?1",
            [course_id],
            |r| r.get(0),
        )
        .optional()?;
    id.map(|id| read_path(conn, &id)).transpose()
}

pub fn current_configuration(
    conn: &Connection,
    course_id: &str,
) -> Result<Option<EnrollmentConfiguration>> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT configuration_json FROM classes WHERE course_id=?1",
            [course_id],
            |r| r.get(0),
        )
        .optional()?;
    raw.map(|raw| serde_json::from_str(&raw).map_err(DbError::from))
        .transpose()
}

/// Keep mutable class settings aligned while the subject adapters own sessions.
/// The accepted entry profile and past path revisions stay immutable.
pub fn sync_configuration(conn: &Connection, course_id: &str, today: &str) -> Result<()> {
    let Some(mut config) = current_configuration(conn, course_id)? else {
        return Ok(());
    };
    let program = crate::classroom::program_row(conn, course_id).map_err(DbError::Invalid)?;
    config.tutor.provider = program.agent;
    config.tutor.model = program.model;
    config.tutor.custom_agent_bin =
        (!program.custom_agent_bin.is_empty()).then_some(program.custom_agent_bin);
    config.pace.session_minutes = program.session_minutes as u32;
    config.pace.weekly_minutes =
        (program.target_weekly_minutes > 0).then_some(program.target_weekly_minutes as u32);
    match &mut config.goal {
        LearningGoal::CourseOutcome { note } => *note = program.learning_goal,
        LearningGoal::LanguageLevel { target_level, note } => {
            let language =
                crate::language::program_view(conn, course_id, today).map_err(DbError::Invalid)?;
            *target_level = language.target_level;
            *note = program.learning_goal;
            config.pace.weekly_minutes = Some(language.weekly_minutes as u32);
        }
    }
    conn.execute(
        "UPDATE classes SET configuration_json=?2,status=?3,updated_at=?4 WHERE course_id=?1",
        params![
            course_id,
            serde_json::to_string(&config)?,
            if program.enabled { "active" } else { "paused" },
            chrono::Utc::now().to_rfc3339()
        ],
    )?;
    Ok(())
}

/// Validate the exact recommendation again inside the same write transaction.
/// Duplicate acceptance returns that immutable revision, even after a later edit.
pub fn accept(conn: &Connection, input: &AcceptPath, today: &str) -> Result<AcceptedPath> {
    let tx = rusqlite::Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let draft = enrollment::draft(&tx, &input.draft_id)?;
    if draft.status == DraftStatus::Accepted {
        let id: Option<String> = tx.query_row("SELECT id FROM path_revisions WHERE class_id = ?1 AND json_extract(plan_json,'$.id') = ?2 AND json_extract(plan_json,'$.draft_revision') = ?3",
            params![draft.accepted_class_id,input.recommendation_id,input.expected_revision], |r| r.get(0)).optional()?;
        return id.map(|id| read_path(&tx, &id)).unwrap_or_else(|| {
            Err(DbError::Invalid(
                "This draft was accepted with a different recommendation.".into(),
            ))
        });
    }
    let recommendation =
        placement::recommend_in_transaction(&tx, &input.draft_id, input.expected_revision)?;
    if recommendation.id != input.recommendation_id {
        return Err(DbError::Invalid(
            "The recommendation changed. Review the current path before accepting it.".into(),
        ));
    }
    let course = catalog::COURSES
        .iter()
        .find(|c| c.course_id == draft.course.course_id)
        .ok_or_else(|| DbError::InvalidFocus(draft.course.course_id.clone()))?;
    if draft.configuration.focus_policy != enrollment::FocusPolicy::Advisory {
        return Err(DbError::Invalid(
            "Class paths currently support advisory focus. Choose advisory before accepting."
                .into(),
        ));
    }
    if !crate::agents::valid_model(&draft.configuration.tutor.model) {
        return Err(DbError::Invalid(
            "Choose a valid model ID before accepting the path.".into(),
        ));
    }
    if crate::agents::RunnerId::parse(&draft.configuration.tutor.provider)
        == Some(crate::agents::RunnerId::CustomCli)
    {
        crate::agents::process::command_words(
            draft
                .configuration
                .tutor
                .custom_agent_bin
                .as_deref()
                .unwrap_or(""),
        )
        .map_err(|e| DbError::Invalid(e.to_string()))?;
    }
    let previous = current_path(&tx, course.course_id)?;
    let class_id = previous
        .as_ref()
        .map(|p| p.reference.class_id.clone())
        .unwrap_or_else(|| format!("class-{:032x}", rand::random::<u128>()));
    let revision = previous.as_ref().map_or(1, |p| p.revision + 1);
    let path_id = format!("path-{:032x}", rand::random::<u128>());
    let now = chrono::Utc::now().to_rfc3339();
    let configuration_json = serde_json::to_string(&draft.configuration)?;
    // Session readers and due/consumed readers continue to use the same subject
    // adapters. This bridge enables that existing program atomically with its path.
    let changed = tx.execute("UPDATE classroom_programs SET enabled=1, agent=?2, model=?3, custom_agent_bin=?4, session_minutes=?5, learning_goal=?6, target_weekly_minutes=COALESCE(?7,target_weekly_minutes), updated_at=?8 WHERE subject_id=?1",
        params![course.id,draft.configuration.tutor.provider,draft.configuration.tutor.model,draft.configuration.tutor.custom_agent_bin.as_deref().unwrap_or(""),draft.configuration.pace.session_minutes,
            match &draft.configuration.goal { LearningGoal::CourseOutcome{note} | LearningGoal::LanguageLevel{note,..} => note },draft.configuration.pace.weekly_minutes,now])?;
    if changed != 1 {
        return Err(DbError::Invalid(
            "Classroom data is not initialized. Restart the app and retry.".into(),
        ));
    }
    if let LearningGoal::LanguageLevel { target_level, .. } = &draft.configuration.goal {
        // Existing lesson/assessment rows and the original progress baseline are
        // retained. An explicit new path changes the cursor for subsequent work.
        let changed = tx.execute("UPDATE language_programs SET enabled=1, current_level=?2, target_level=?3, start_level=CASE WHEN EXISTS(SELECT 1 FROM language_sessions WHERE language=?1) THEN start_level ELSE ?2 END, start_date=CASE WHEN EXISTS(SELECT 1 FROM language_sessions WHERE language=?1) THEN start_date ELSE ?7 END, weekly_minutes=COALESCE(?4,weekly_minutes), session_minutes=?5, updated_at=?6 WHERE language=?1",
            params![course.id,recommendation.entry_point,target_level,draft.configuration.pace.weekly_minutes,draft.configuration.pace.session_minutes,now,today])?;
        if changed != 1 {
            return Err(DbError::Invalid(
                "Language data is not initialized. Restart the app and retry.".into(),
            ));
        }
    }
    tx.execute("INSERT INTO classes(id,course_id,course_snapshot_fingerprint,status,configuration_json,active_path_revision_id,created_at,updated_at) VALUES(?1,?2,?3,'active',?4,?5,?6,?6) ON CONFLICT(course_id) DO UPDATE SET course_snapshot_fingerprint=excluded.course_snapshot_fingerprint, configuration_json=excluded.configuration_json,active_path_revision_id=excluded.active_path_revision_id,status='active',updated_at=excluded.updated_at",
        params![class_id,course.course_id,draft.course.fingerprint,configuration_json,path_id,now])?;
    tx.execute("INSERT INTO path_revisions(id,class_id,revision,course_snapshot_fingerprint,entry_profile_json,plan_json,accepted_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![path_id,class_id,revision,draft.course.fingerprint,configuration_json,serde_json::to_string(&recommendation)?,now])?;
    tx.execute("UPDATE enrollment_drafts SET status='accepted',accepted_class_id=?2,updated_at=?3 WHERE id=?1",params![draft.id.0,class_id,now])?;
    tx.execute("INSERT OR IGNORE INTO legacy_crosswalk(legacy_table,legacy_key,entity_kind,entity_id,imported_at) VALUES('classroom_programs',?1,'class',?2,?3)",params![course.id,class_id,now])?;
    let result = read_path(&tx, &path_id)?;
    tx.commit()?;
    Ok(result)
}

/// A manual/diagnostic bypass changes the route, not the mastery ledger. Earlier
/// prerequisites are carried into the lesson as visible checks and refreshers.
pub fn next_concept(conn: &Connection, course_id: &str, date: &str) -> Result<Option<db::Concept>> {
    let Some(path) = current_path(conn, course_id)? else {
        return crate::roulette::draw(conn, date, course_id);
    };
    if enrollment::course_snapshot(course_id)?.0.fingerprint
        != path.reference.course_snapshot_fingerprint
    {
        return Err(DbError::Invalid(
            "The curriculum changed. Review a new path before starting another lesson.".into(),
        ));
    }
    let earlier: std::collections::HashSet<_> = path
        .recommendation
        .earlier_topics
        .iter()
        .map(|p| p.id.as_str())
        .collect();
    let mut states = std::collections::HashMap::new();
    let mut statement = conn.prepare("SELECT c.slug,COALESCE(m.state,'unseen'),c.prereqs_json FROM concepts c LEFT JOIN mastery m ON m.concept_id=c.id WHERE c.active=1 AND c.focus=?1")?;
    let rows = statement.query_map([course_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    let mut prerequisites = std::collections::HashMap::new();
    for row in rows {
        let (slug, state, raw) = row?;
        prerequisites.insert(slug.clone(), serde_json::from_str::<Vec<String>>(&raw)?);
        states.insert(slug, state);
    }
    let mut candidates: Vec<_> = db::all_concepts(conn, course_id)?
        .into_iter()
        .filter(|c| {
            !earlier.contains(c.slug.as_str())
                && !states
                    .get(&c.slug)
                    .is_some_and(|state| crate::roulette::is_completed(state))
        })
        .filter(|c| {
            prerequisites.get(&c.slug).is_none_or(|reqs| {
                reqs.iter().all(|slug| {
                    earlier.contains(slug.as_str())
                        || states.get(slug).is_some_and(|s| {
                            matches!(
                                s.as_str(),
                                "practicing"
                                    | "struggling"
                                    | "mastered"
                                    | "maintenance"
                                    | "decayed"
                            )
                        })
                })
            })
        })
        .collect();
    let order = |phase: &str| {
        [
            "foundations",
            "mechanisms",
            "production",
            "synthesis",
            "elective",
        ]
        .iter()
        .position(|p| *p == phase)
        .unwrap_or(5)
    };
    candidates.sort_by_key(|c| {
        (
            !c.curriculum.core,
            order(&c.curriculum.phase),
            c.times_picked,
            c.tier,
            c.id,
        )
    });
    let next = candidates.into_iter().next();
    if let Some(concept) = &next {
        db::mark_concept_picked(conn, concept.id, date)?;
    }
    Ok(next)
}

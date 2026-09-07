use crate::db::{self, Session};
use crate::focus;
use crate::generator::GeneratedQuestion;
use crate::state::AppState;
use rusqlite::params;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

pub const STEP_QUIZ: &str = "quiz";
pub const STEP_REVIEW: &str = "review";
pub const STEP_ROULETTE: &str = "roulette";
pub const STEP_COURSE: &str = "course";
pub const STEP_DONE: &str = "done";

#[derive(Debug, Clone, Serialize)]
pub struct SessionView {
    pub date: String,
    pub status: String,
    pub step: String,
    pub quiz_score: Option<f64>,
    pub streak: i64,
    pub locked: bool,
    pub session_type: String,
    pub plan_reason: String,
    pub focus: Option<String>,
}

pub fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Is a session owed right now? (scheduled time passed, today not completed/skipped)
pub fn session_owed(state: &AppState) -> bool {
    let today = state.today();
    let conn = state.db.0.lock().unwrap();
    let onboarded = matches!(db::get_config(&conn, "onboarded"), Ok(Some(v)) if v == "1");
    if !onboarded {
        return false;
    }
    // Paused scheduler: nothing is ever owed until the user resumes.
    if matches!(db::get_config(&conn, "schedule_paused"), Ok(Some(v)) if v == "1") {
        return false;
    }
    if let Ok(Some(s)) = db::get_session(&conn, &today) {
        if s.status == "completed" || s.status == "skipped" {
            return false;
        }
    }
    // A voluntary extension re-opens the session but is never owed — the
    // kiosk must not re-engage on extra topics the user chose to do.
    if matches!(db::get_config(&conn, &format!("extended:{today}")), Ok(Some(v)) if v == "1") {
        return false;
    }
    if state.debug_day {
        return true;
    }
    let configured_hour: u32 = db::get_config(&conn, "schedule_hour")
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9);
    let configured_minute: u32 = db::get_config(&conn, "schedule_minute")
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let (hour, minute) = if configured_hour <= 23 && configured_minute <= 59 {
        (configured_hour, configured_minute)
    } else {
        log::error!(
            "invalid persisted schedule {configured_hour:02}:{configured_minute:02}; using 09:00"
        );
        (9, 0)
    };
    let now = chrono::Local::now();
    let sched = now
        .date_naive()
        .and_hms_opt(hour, minute, 0)
        .expect("validated schedule time");
    now.naive_local() >= sched
}

/// Get or create today's session row and return it.
pub fn ensure_today_session(state: &AppState) -> db::Result<Session> {
    let today = state.today();
    let conn = state.db.0.lock().unwrap();
    if let Some(s) = db::get_session(&conn, &today)? {
        return Ok(s);
    }
    let s = Session {
        date: today,
        concept_id: None,
        status: "pending".into(),
        current_step: STEP_QUIZ.into(),
        quiz_score: None,
        started_at: None,
        completed_at: None,
        reading_seconds: 0,
        session_type: "lesson".into(),
        plan_reason: String::new(),
        focus: String::new(),
    };
    db::upsert_session(&conn, &s)?;
    Ok(s)
}

/// Target size for a pop-quiz day's question set.
const POP_QUIZ_SIZE: i64 = 12;

fn pop_quiz_key(date: &str) -> String {
    format!("pop_quiz_set:{date}")
}

/// Resolve the persisted focus for a session date. Empty means not chosen yet.
pub fn session_focus(conn: &rusqlite::Connection, date: &str) -> db::Result<Option<String>> {
    Ok(db::get_session(conn, date)?.and_then(|s| {
        if s.focus.is_empty() {
            None
        } else {
            Some(s.focus)
        }
    }))
}

/// Apply focus on session start. New sessions require a selectable focus;
/// in-progress sessions keep their persisted focus (conflicting input is ignored).
fn apply_session_focus(s: &mut Session, chosen: Option<&str>) -> db::Result<()> {
    if s.status == "completed" || s.status == "skipped" {
        return Ok(());
    }
    if !s.focus.is_empty() {
        if let Some(f) = chosen {
            if f != s.focus {
                // Safe ignore: resume with the persisted track.
                return Ok(());
            }
        }
        return Ok(());
    }
    let focus = chosen.ok_or(db::DbError::FocusRequired)?;
    focus::validate_selectable(focus)?;
    s.focus = focus.to_string();
    Ok(())
}

/// Today's question set, honoring the session type and focus track.
pub fn questions_for_today(
    conn: &rusqlite::Connection,
    today: &str,
    yesterday: &str,
) -> db::Result<Vec<db::Question>> {
    let focus = session_focus(conn, today)?.unwrap_or_default();
    if focus.is_empty() {
        return Ok(Vec::new());
    }
    let is_pop = db::get_session(conn, today)?
        .map(|s| s.session_type == "pop_quiz")
        .unwrap_or(false);
    if is_pop {
        if let Ok(Some(ids_json)) = db::get_config(conn, &pop_quiz_key(today)) {
            let ids: Vec<i64> = serde_json::from_str(&ids_json).unwrap_or_default();
            let mut out = Vec::new();
            for id in ids {
                let mut stmt = conn.prepare(
                    "SELECT id, course_id, prompt, kind, choices_json, correct_answer, explanation,
                            CASE WHEN id IN (SELECT question_id FROM carryover) THEN 'carryover' ELSE origin END
                     FROM questions WHERE id = ?1",
                )?;
                let mut rows = stmt.query([id])?;
                if let Some(r) = rows.next()? {
                    out.push(db::Question {
                        id: r.get(0)?,
                        course_id: r.get(1)?,
                        prompt: r.get(2)?,
                        kind: r.get(3)?,
                        choices_json: r.get(4)?,
                        correct_answer: r.get(5)?,
                        explanation: r.get(6)?,
                        origin: r.get(7)?,
                    });
                }
            }
            return Ok(out);
        }
    }
    let mut quiz = db::quiz_for_date(conn, today, yesterday, &focus)?;
    if !is_pop {
        let exclude: Vec<i64> = quiz.iter().map(|question| question.id).collect();
        let review = db::spaced_review_sample(conn, today, &focus, &exclude, 2)?;
        quiz.extend(review);
    }
    Ok(quiz)
}

/// Begin (or resume) today's session: pick the right starting step.
pub fn start_session(state: &AppState, chosen_focus: Option<&str>) -> db::Result<Session> {
    let mut s = ensure_today_session(state)?;
    if s.status == "completed" || s.status == "skipped" {
        return Ok(s);
    }
    apply_session_focus(&mut s, chosen_focus)?;
    let today = state.today();
    let yesterday = state.yesterday();
    let focus = s.focus.clone();
    let conn = state.db.0.lock().unwrap();
    if s.status == "pending" {
        // Older builds may have pre-drawn a system-design concept before the
        // learner chose today's focus. Never carry that concept across tracks.
        if let Some(concept_id) = s.concept_id {
            let matches_focus = db::get_concept(&conn, concept_id)?
                .map(|concept| concept.focus == focus)
                .unwrap_or(false);
            if !matches_focus {
                s.concept_id = None;
            }
        }
        if s.session_type == "pop_quiz" {
            // Freeze the audit set: carryover + yesterday's fresh + breadth sample.
            let base = db::quiz_for_date(&conn, &today, &yesterday, &focus)?;
            let base_ids: Vec<i64> = base.iter().map(|q| q.id).collect();
            let extra = db::pop_quiz_sample(
                &conn,
                &today,
                &focus,
                &base_ids,
                (POP_QUIZ_SIZE - base.len() as i64).max(0),
            )?;
            let all_ids: Vec<i64> = base_ids
                .iter()
                .chain(extra.iter().map(|q| &q.id))
                .copied()
                .collect();
            if all_ids.is_empty() {
                // Nothing to audit (shouldn't happen given planner guardrails):
                // degrade to a normal lesson day.
                s.session_type = "lesson".into();
                s.plan_reason = String::new();
            } else {
                db::set_config(
                    &conn,
                    &pop_quiz_key(&today),
                    &serde_json::to_string(&all_ids)?,
                )?;
            }
        }
        let quiz = questions_for_today(&conn, &today, &yesterday)?;
        s.current_step = if quiz.is_empty() {
            STEP_ROULETTE.into()
        } else {
            STEP_QUIZ.into()
        };
        s.status = "in_progress".into();
        s.started_at = Some(now_iso());
        db::upsert_session(&conn, &s)?;
        drop(conn);
        state.clear_chat_threads();
        // Focus is only known now. Start course generation in the background
        // while the learner takes the quiz or watches the roulette.
        if s.session_type == "lesson" {
            let conn = state.db.0.lock().unwrap();
            db::jobs::requeue(&conn, "course", &today)?;
            state.gen_notify.notify_one();
        }
    }
    Ok(s)
}

pub fn set_step(state: &AppState, step: &str) -> db::Result<Session> {
    let today = state.today();
    let conn = state.db.0.lock().unwrap();
    let mut s = db::get_session(&conn, &today)?.expect("session exists");
    s.current_step = step.to_string();
    db::upsert_session(&conn, &s)?;
    Ok(s)
}

/// Validated forward-only transitions.
pub fn allowed_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        (STEP_QUIZ, STEP_REVIEW)
            | (STEP_REVIEW, STEP_ROULETTE)
            | (STEP_ROULETTE, STEP_COURSE)
            | (STEP_COURSE, STEP_DONE)
    )
}

pub fn view(state: &AppState) -> SessionView {
    let today = state.today();
    let conn = state.db.0.lock().unwrap();
    let s = db::get_session(&conn, &today).ok().flatten();
    let streak = db::streak(&conn, &today).unwrap_or(0);
    match s {
        Some(s) => SessionView {
            date: s.date,
            status: s.status,
            step: s.current_step,
            quiz_score: s.quiz_score,
            streak,
            locked: state.locked.load(std::sync::atomic::Ordering::SeqCst),
            session_type: s.session_type,
            plan_reason: s.plan_reason,
            focus: if s.focus.is_empty() {
                None
            } else {
                Some(s.focus)
            },
        },
        None => SessionView {
            date: today,
            status: "pending".into(),
            step: STEP_QUIZ.into(),
            quiz_score: None,
            streak,
            locked: false,
            session_type: "lesson".into(),
            plan_reason: String::new(),
            focus: None,
        },
    }
}

/// Mark today completed, enqueue tomorrow's pregeneration, release the lock.
pub fn complete_session(app: &AppHandle, state: &AppState) -> db::Result<Session> {
    let today = state.today();
    let tomorrow = state.tomorrow();
    let s = {
        let conn = state.db.0.lock().unwrap();
        let mut s = db::get_session(&conn, &today)?.expect("session exists");
        s.status = "completed".into();
        s.current_step = STEP_DONE.into();
        s.completed_at = Some(now_iso());
        db::upsert_session(&conn, &s)?;
        // Mastery ledger: today's concept has been read (unseen -> introduced).
        if let Some(cid) = s.concept_id {
            let _ = crate::mastery::record_course_read(&conn, cid, &today);
        }
        db::jobs::enqueue(&conn, "quiz", &tomorrow)?;
        s
    };
    state.gen_notify.notify_one();
    state.clear_chat_threads();
    crate::kiosk::release(app, state);
    let _ = app.emit("session:state", view(state));
    Ok(s)
}

/// Store generated quiz questions for `target_date`, linked to the course studied the day before.
pub fn persist_quiz_questions(
    conn: &rusqlite::Connection,
    course_id: i64,
    questions: &[GeneratedQuestion],
) -> db::Result<()> {
    for q in questions {
        let choices = q.choices.as_ref().map(serde_json::to_string).transpose()?;
        db::insert_question(
            conn,
            course_id,
            &q.prompt,
            if q.kind == "mcq" { "mcq" } else { "free" },
            choices.as_deref(),
            &q.correct_answer,
            &q.explanation,
        )?;
    }
    Ok(())
}

/// Background generation worker: drains the generation_jobs queue forever.
pub async fn generation_worker(app: AppHandle) {
    let state = app.state::<AppState>();
    loop {
        let job = {
            let conn = state.db.0.lock().unwrap();
            db::jobs::next_queued(&conn).ok().flatten()
        };
        let Some((job_id, kind, target_date)) = job else {
            tokio::select! {
                _ = state.gen_notify.notified() => {},
                _ = tokio::time::sleep(std::time::Duration::from_secs(3600)) => {},
            }
            continue;
        };
        {
            let conn = state.db.0.lock().unwrap();
            let _ = db::jobs::mark(&conn, job_id, "running", None);
        }
        let _ = app.emit("gen:status", format!("generating {kind} for {target_date}"));
        let result = run_generation_job(&app, &state, &kind, &target_date).await;
        {
            let conn = state.db.0.lock().unwrap();
            match &result {
                Ok(_) => {
                    let _ = db::jobs::mark(&conn, job_id, "done", None);
                }
                Err(e) => {
                    let _ = db::jobs::mark(&conn, job_id, "failed", Some(&e.to_string()));
                    // failed jobs retry on next wakeup (attempts capped in next_queued)
                    let _ = db::jobs::enqueue(&conn, &kind, &target_date);
                }
            }
        }
        let _ = app.emit(
            "gen:status",
            match &result {
                Ok(_) => format!("{kind} for {target_date} ready"),
                Err(e) => format!("{kind} for {target_date} failed: {e}"),
            },
        );
    }
}

/// Minimum quizzed concepts before pop-quiz days become possible.
const POP_QUIZ_MIN_PRACTICED: i64 = 6;

pub fn weekly_review_due(completed_in_track: i64) -> bool {
    completed_in_track > 0 && completed_in_track % 7 == 0
}

/// Decide (once) what kind of day `date` is. Requires a chosen focus; without
/// one the caller must not pre-plan or pre-generate content.
pub async fn ensure_day_plan(
    state: &tauri::State<'_, AppState>,
    date: &str,
    track_focus: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let planned_key = format!("planned:{date}");
    let (eligible, milestone_review, dossier) = {
        let conn = state.db.0.lock().unwrap();
        let already = db::get_config(&conn, &planned_key)?.is_some();
        let existing_type = db::get_session(&conn, date)?.map(|s| s.session_type);
        if already {
            return Ok(existing_type.unwrap_or_else(|| "lesson".into()));
        }
        let practiced: i64 = conn.query_row(
            "SELECT COUNT(*) FROM mastery m
             JOIN concepts c ON c.id = m.concept_id
             WHERE c.focus = ?1
               AND m.state IN ('practicing','struggling','mastered','maintenance','decayed')",
            params![track_focus],
            |r| r.get(0),
        )?;
        let prev = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")?
            .pred_opt()
            .ok_or("date underflow")?
            .format("%Y-%m-%d")
            .to_string();
        let prev_was_pop = db::get_session(&conn, &prev)?
            .map(|s| s.session_type == "pop_quiz")
            .unwrap_or(false);
        let eligible = practiced >= POP_QUIZ_MIN_PRACTICED && !prev_was_pop;
        let completed_in_track: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE status = 'completed' AND focus = ?1",
            params![track_focus],
            |r| r.get(0),
        )?;
        let milestone_review = weekly_review_due(completed_in_track);
        let dossier = crate::mastery::build_dossier(&conn, date, track_focus).unwrap_or_default();
        (eligible, milestone_review, dossier)
    };

    // Test/debug override skips the agent call entirely.
    let plan = if let Ok(forced) = std::env::var("SDR_SESSION_TYPE") {
        crate::generator::SessionPlan {
            session_type: forced,
            reason: "forced via SDR_SESSION_TYPE".into(),
        }
    } else if milestone_review && eligible {
        crate::generator::SessionPlan {
            session_type: "pop_quiz".into(),
            reason:
                "weekly retrieval checkpoint — integrate and strengthen the last seven sessions"
                    .into(),
        }
    } else if !eligible {
        crate::generator::SessionPlan {
            session_type: "lesson".into(),
            reason: String::new(),
        }
    } else {
        state
            .generator
            .plan_day(&dossier, eligible, track_focus)
            .await
    };
    // Guardrails beat the model.
    let session_type = if plan.session_type == "pop_quiz" && eligible {
        "pop_quiz"
    } else {
        "lesson"
    };

    let conn = state.db.0.lock().unwrap();
    let mut s = db::get_session(&conn, date)?.unwrap_or(Session {
        date: date.to_string(),
        concept_id: None,
        status: "pending".into(),
        current_step: STEP_QUIZ.into(),
        quiz_score: None,
        started_at: None,
        completed_at: None,
        reading_seconds: 0,
        session_type: "lesson".into(),
        plan_reason: String::new(),
        focus: track_focus.to_string(),
    });
    // Never re-type a session already underway.
    if s.status == "pending" {
        s.session_type = session_type.to_string();
        s.plan_reason = plan.reason.chars().take(120).collect();
        db::upsert_session(&conn, &s)?;
    }
    db::set_config(&conn, &planned_key, "1")?;
    Ok(s.session_type)
}

async fn run_generation_job(
    _app: &AppHandle,
    state: &tauri::State<'_, AppState>,
    kind: &str,
    target_date: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match kind {
        "course" => {
            let track_focus = {
                let conn = state.db.0.lock().unwrap();
                session_focus(&conn, target_date)?
            };
            let Some(track_focus) = track_focus else {
                // Focus is chosen at session start — never pre-draw tomorrow's course.
                return Ok(());
            };
            let session_type = ensure_day_plan(state, target_date, &track_focus).await?;
            if session_type == "pop_quiz" {
                return Ok(());
            }
            let course = ensure_course_for_date(state, target_date).await?;
            // Audio mode: script (and, if provisioned, VibeVoice render)
            // happen in this same overnight window. Failures never block.
            let audio_on = {
                let conn = state.db.0.lock().unwrap();
                matches!(db::get_config(&conn, "audio_enabled"), Ok(Some(v)) if v == "1")
            };
            if audio_on {
                if let Err(e) = ensure_audio_for_course(state, &course, target_date).await {
                    log::warn!("audio prep failed (session unaffected): {e}");
                }
            }
            Ok(())
        }
        "quiz" => {
            // Quiz for target_date tests the course read the day before it.
            let prev = chrono::NaiveDate::parse_from_str(target_date, "%Y-%m-%d")?
                .pred_opt()
                .ok_or("date underflow")?
                .format("%Y-%m-%d")
                .to_string();
            let (course, prev_was_pop) = {
                let conn = state.db.0.lock().unwrap();
                (
                    db::course_for_date(&conn, &prev)?,
                    db::get_session(&conn, &prev)?
                        .map(|s| s.session_type == "pop_quiz")
                        .unwrap_or(false),
                )
            };
            let Some(course) = course else {
                if prev_was_pop {
                    // Pop-quiz days produce no course; nothing to quiz on. Fine.
                    return Ok(());
                }
                return Err(format!("no course for {prev}, cannot build quiz").into());
            };
            let (track_focus, concept_title) = {
                let conn = state.db.0.lock().unwrap();
                db::get_concept(&conn, course.concept_id)?
                    .map(|concept| (concept.focus, concept.title))
                    .ok_or("course concept missing")?
            };
            let (existing, dossier) = {
                let conn = state.db.0.lock().unwrap();
                (
                    db::questions_for_course(&conn, course.id)?,
                    crate::mastery::build_dossier(&conn, target_date, &track_focus)
                        .unwrap_or_default(),
                )
            };
            if !existing.is_empty() {
                return Ok(());
            }
            let (questions, _src) = state
                .generator
                .generate_quiz(&course.markdown, &dossier, &track_focus, &concept_title)
                .await?;
            let conn = state.db.0.lock().unwrap();
            persist_quiz_questions(&conn, course.id, &questions)?;
            Ok(())
        }
        other => Err(format!("unknown job kind {other}").into()),
    }
}

/// Script (always) + VibeVoice render (when the venv is provisioned) for a course.
pub async fn ensure_audio_for_course(
    state: &tauri::State<'_, AppState>,
    course: &db::Course,
    date: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (existing, focus) = {
        let conn = state.db.0.lock().unwrap();
        let focus = db::get_concept(&conn, course.concept_id)?
            .map(|concept| concept.focus)
            .ok_or("course concept missing")?;
        (crate::audio::get_script(&conn, course.id)?, focus)
    };
    let lines = match existing {
        Some(v) => v.lines,
        None => {
            let lines = state
                .generator
                .generate_audio_script(&course.markdown, &focus)
                .await?;
            if lines.is_empty() {
                return Err("empty audio script".into());
            }
            let conn = state.db.0.lock().unwrap();
            crate::audio::save_script(&conn, course.id, &lines)?;
            lines
        }
    };
    if crate::audio::vibevoice_python(&state.data_dir).is_some() {
        let already_rendered = lines.iter().any(|l| l.file.is_some());
        if !already_rendered {
            let dir = crate::audio::render_vibevoice(&state.data_dir, date, &lines)
                .await
                .map_err(|e| format!("vibevoice render: {e}"))?;
            let conn = state.db.0.lock().unwrap();
            crate::audio::mark_rendered(&conn, course.id, &dir)?;
        }
    }
    Ok(())
}

/// Make sure a course exists for `date`: draw a concept if needed, generate, persist.
pub async fn ensure_course_for_date(
    state: &tauri::State<'_, AppState>,
    date: &str,
) -> Result<db::Course, Box<dyn std::error::Error + Send + Sync>> {
    let track_focus = {
        let conn = state.db.0.lock().unwrap();
        session_focus(&conn, date)?
    };
    let track_focus = track_focus.ok_or("session focus not chosen yet")?;
    {
        let conn = state.db.0.lock().unwrap();
        if let Some(c) = db::course_for_date(&conn, date)? {
            let session = db::get_session(&conn, date)?;
            let session_concept = session.as_ref().and_then(|s| s.concept_id);
            let concept = db::get_concept(&conn, c.concept_id)?.ok_or("concept missing")?;
            let focus_ok = concept.focus == track_focus;
            if focus_ok && (session_concept.is_none() || session_concept == Some(c.concept_id)) {
                return Ok(c);
            }
        }
    }
    // Draw (or reuse) the concept for that date's session.
    let concept = {
        let conn = state.db.0.lock().unwrap();
        let existing = db::get_session(&conn, date)?;
        let concept_id = existing.as_ref().and_then(|s| s.concept_id);
        match concept_id {
            Some(id) => {
                let c = db::get_concept(&conn, id)?.ok_or("concept missing")?;
                if c.focus != track_focus {
                    return Err(format!(
                        "session concept focus {} does not match session focus {track_focus}",
                        c.focus
                    )
                    .into());
                }
                c
            }
            None => {
                let c = crate::roulette::draw(&conn, date, &track_focus)?
                    .ok_or("empty concept pool")?;
                let mut s = existing.unwrap_or(Session {
                    date: date.to_string(),
                    concept_id: None,
                    status: "pending".into(),
                    current_step: STEP_QUIZ.into(),
                    quiz_score: None,
                    started_at: None,
                    completed_at: None,
                    reading_seconds: 0,
                    session_type: "lesson".into(),
                    plan_reason: String::new(),
                    focus: track_focus.clone(),
                });
                s.concept_id = Some(c.id);
                db::upsert_session(&conn, &s)?;
                c
            }
        }
    };
    let dossier = {
        let conn = state.db.0.lock().unwrap();
        crate::mastery::build_dossier(&conn, date, &track_focus).unwrap_or_default()
    };
    let (course, source) = state
        .generator
        .generate_course(crate::generator::CourseRequest {
            title: &concept.title,
            category: &concept.category,
            dossier: &dossier,
            focus: &track_focus,
            curriculum: &concept.curriculum,
        })
        .await?;
    let resources_json = serde_json::to_string(&course.resources)?;
    let course_row = {
        let conn = state.db.0.lock().unwrap();
        let id = db::insert_course(
            &conn,
            date,
            concept.id,
            &course.markdown,
            &resources_json,
            &source,
        )?;
        for q in &course.exit_questions {
            let _ = db::insert_exit_question(
                &conn,
                id,
                1,
                &q.prompt,
                &serde_json::to_string(&q.choices)?,
                &q.correct_answer,
                &q.explanation,
                &q.section,
                &q.learning_objective,
            );
        }
        if let Some(exercise) = &course.exercise {
            let _ = db::upsert_course_exercise(
                &conn,
                id,
                &exercise.title,
                &exercise.instructions,
                exercise.starter_code.as_deref(),
                exercise.deliverable.as_deref(),
                &exercise.hints,
            );
        }
        db::course_for_date(&conn, date)?.ok_or("course vanished")?
    };
    // Mirror to a human-greppable markdown file.
    let mirror = state
        .data_dir
        .join("courses")
        .join(format!("{}-{}.md", date, concept.slug));
    let _ = std::fs::create_dir_all(mirror.parent().unwrap());
    let _ = std::fs::write(&mirror, &course_row.markdown);
    Ok(course_row)
}

/// Authoritative course reading timer. Emits timer:tick {remaining} every second.
fn timer_tick_is_active(paused: bool, elapsed: std::time::Duration) -> bool {
    !paused && elapsed < std::time::Duration::from_secs(5)
}

pub async fn run_course_timer(app: AppHandle) {
    use std::sync::atomic::Ordering;
    let state = app.state::<AppState>();
    if state.timer_running.swap(true, Ordering::SeqCst) {
        return;
    }
    let mut persist_counter = 0;
    let mut last_tick = std::time::Instant::now();
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let elapsed = last_tick.elapsed();
        last_tick = std::time::Instant::now();
        let paused = state.timer_paused.load(Ordering::SeqCst);
        if !timer_tick_is_active(paused, elapsed) {
            if elapsed >= std::time::Duration::from_secs(5) {
                log::info!(
                    "reading timer detected a {:.1}s sleep/wake gap; elapsed sleep is not charged",
                    elapsed.as_secs_f64()
                );
                let _ = app.emit("timer:paused-for-sleep", elapsed.as_secs());
            }
            continue;
        }
        let remaining = state.reading_remaining.load(Ordering::SeqCst);
        if remaining <= 0 {
            let _ = app.emit("timer:tick", 0);
            break;
        }
        let next = remaining - 1;
        state.reading_remaining.store(next, Ordering::SeqCst);
        let _ = app.emit("timer:tick", next);
        persist_counter += 1;
        if persist_counter % 10 == 0 {
            let today = state.today();
            let total = state.course_duration_secs();
            let conn = state.db.0.lock().unwrap();
            if let Ok(Some(mut s)) = db::get_session(&conn, &today) {
                s.reading_seconds = total - next;
                let _ = db::upsert_session(&conn, &s);
            }
        }
        if next <= 0 {
            break;
        }
    }
    state.timer_running.store(false, Ordering::SeqCst);
    let _ = app.emit("timer:done", true);
}

#[cfg(test)]
mod timer_tests {
    use super::timer_tick_is_active;
    use std::time::Duration;

    #[test]
    fn reading_timer_never_charges_explicit_pause_or_sleep_gap() {
        assert!(timer_tick_is_active(false, Duration::from_secs(1)));
        assert!(!timer_tick_is_active(true, Duration::from_secs(1)));
        assert!(!timer_tick_is_active(false, Duration::from_secs(30)));
    }
}

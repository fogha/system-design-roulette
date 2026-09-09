pub mod agents;
use crate::db::{self, Attempt};
use crate::domain::{
    assessments::{self, Owner, ResponseStatus, Round, RoundId},
    primary_quiz,
};
use crate::generator::GradeItem;
use crate::session::{self, SessionView};
use crate::state::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, State};

type CmdResult<T> = Result<T, String>;
pub mod placement;

#[tauri::command]
pub fn get_enrollment_options(
    course_id: String,
) -> CmdResult<crate::domain::enrollment::EnrollmentOptions> {
    crate::domain::enrollment::options(&course_id).map_err(err)
}

#[tauri::command]
pub fn get_enrollment_draft(
    state: State<'_, AppState>,
    course_id: String,
) -> CmdResult<Option<crate::domain::enrollment::EnrollmentDraft>> {
    crate::domain::enrollment::draft_for_course(&state.db.0.lock().unwrap(), &course_id)
        .map_err(err)
}

#[tauri::command]
pub fn save_enrollment_draft(
    state: State<'_, AppState>,
    input: crate::domain::enrollment::SaveEnrollmentDraft,
) -> CmdResult<crate::domain::enrollment::EnrollmentDraft> {
    crate::domain::enrollment::save_draft(&state.db.0.lock().unwrap(), &input).map_err(err)
}

#[tauri::command]
pub fn get_catalog() -> CmdResult<Vec<crate::catalog::CourseDefinition>> {
    crate::catalog::validate()?;
    Ok(crate::catalog::COURSES.to_vec())
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[derive(Serialize)]
pub struct AppStateView {
    pub onboarded: bool,
    pub session: SessionView,
    pub owed: bool,
    pub schedule_hour: u32,
    pub schedule_minute: u32,
    pub agent_ok: Option<bool>,
    pub debug_day: bool,
    /// ~/sdr-unlock exists: every lock releases instantly. Surfaced so a
    /// leftover emergency file can't silently neuter enforcement.
    pub enforcement_disarmed: bool,
    /// Scheduler paused: no launchd agent, no owed sessions, until resumed.
    pub schedule_paused: bool,
    /// Kiosk strictness: 'advisory' | 'firm' | 'hard'.
    pub kiosk_level: String,
    /// Selected model identifier for the primary runner.
    pub model: String,
    /// Primary generation provider.
    pub agent: String,
    /// Binary path used when agent == 'custom'.
    pub custom_agent_bin: String,
    /// Today's chosen learning track, if the session has started with a focus.
    pub selected_focus: Option<String>,
    /// Whether a DeepSeek API key is available (env var or Keychain) — the
    /// key itself never leaves the Rust process.
    pub deepseek_key_configured: bool,
    /// Generic advisory classroom. Every subject owns its schedule, prompt
    /// profile, generation provider, progress, and same-day sessions.
    /// Languages are classroom subjects too; their CEFR engine lives behind
    /// the same panel.
    pub classroom_programs: Vec<crate::classroom::ClassroomProgramView>,
    pub classroom_slots: Vec<crate::classroom::ClassroomSlotView>,
    pub classroom_due_count: usize,
    pub active_classroom_sessions: Vec<crate::classroom::ActiveClassroomSessionView>,
}

fn valid_agent(agent: &str) -> bool {
    crate::agents::RunnerId::parse(agent).is_some()
}

/// Switch the primary CLI agent (and the custom binary path when relevant).
/// Applies to the next generation; provider switching never creates a fallback.
#[tauri::command]
pub fn set_agent(
    state: State<'_, AppState>,
    agent: String,
    custom_bin: Option<String>,
) -> CmdResult<()> {
    if !valid_agent(&agent) {
        return Err(format!("unknown agent: {agent}"));
    }
    let custom = custom_bin.unwrap_or_default();
    if agent == "custom" && custom.trim().is_empty() {
        return Err("custom agent needs a binary path".into());
    }
    if agent == "custom" {
        let words = crate::agents::process::command_words(&custom).map_err(err)?;
        if crate::agents::process::resolve(&words[0]).is_none() {
            return Err("custom executable not found".into());
        }
    }
    let mut conn = state.db.0.lock().unwrap();
    let tx = conn.transaction().map_err(err)?;
    let runner = crate::agents::RunnerId::parse(&agent).ok_or("unknown runner")?;
    let prior_agent = db::get_config(&tx, "agent")
        .map_err(err)?
        .unwrap_or_else(|| "claude".into());
    let prior_model = db::get_config(&tx, "model")
        .map_err(err)?
        .unwrap_or_else(|| "opus".into());
    if let Some(prior) = crate::agents::RunnerId::parse(&prior_agent) {
        db::set_config(&tx, &format!("model_{}", prior.id()), &prior_model).map_err(err)?;
    }
    let model = db::get_config(&tx, &format!("model_{}", runner.id()))
        .map_err(err)?
        .unwrap_or_else(|| crate::agents::default_model(runner));
    db::set_config(&tx, "model", &model).map_err(err)?;
    db::set_config(&tx, "agent", &agent).map_err(err)?;
    db::set_config(&tx, "custom_agent_bin", custom.trim()).map_err(err)?;
    tx.commit().map_err(err)?;
    let mut current_agent = state.generator.agent.lock().unwrap();
    let mut current_model = state.generator.model.lock().unwrap();
    let mut current_custom = state.generator.custom_bin.lock().unwrap();
    *current_agent = agent;
    *current_model = model;
    *current_custom = custom.trim().into();
    Ok(())
}

fn valid_model(model: &str) -> bool {
    crate::agents::valid_model(model)
}

/// Change the course-generation model. Applies to the NEXT generation —
/// in-flight calls keep the model they started with.
#[tauri::command]
pub fn set_model(state: State<'_, AppState>, model: String) -> CmdResult<()> {
    if !valid_model(&model) {
        return Err(format!("unknown model: {model}"));
    }
    let mut conn = state.db.0.lock().unwrap();
    let tx = conn.transaction().map_err(err)?;
    let runner =
        crate::agents::RunnerId::parse(&state.generator.current_agent()).ok_or("unknown runner")?;
    db::set_config(&tx, "model", &model).map_err(err)?;
    db::set_config(&tx, &format!("model_{}", runner.id()), &model).map_err(err)?;
    tx.commit().map_err(err)?;
    *state.generator.model.lock().unwrap() = model;
    Ok(())
}

fn valid_kiosk_level(level: &str) -> bool {
    matches!(level, "advisory" | "firm" | "hard")
}

/// Change kiosk strictness. Refused while locked — strictness can't be
/// downgraded mid-session.
#[tauri::command]
pub fn set_kiosk_level(state: State<'_, AppState>, level: String) -> CmdResult<()> {
    if !valid_kiosk_level(&level) {
        return Err(format!("unknown kiosk level: {level}"));
    }
    if state.locked.load(Ordering::SeqCst) {
        return Err("cannot change enforcement during a locked session".into());
    }
    let conn = state.db.0.lock().unwrap();
    db::set_config(&conn, "kiosk_level", &level).map_err(err)
}

/// Called by the webview on boot. Until this fires, the kiosk refuses to
/// engage (a dead webview has no escape hatch).
#[tauri::command]
pub fn mark_frontend_ready(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    state.frontend_ready.store(true, Ordering::SeqCst);
    if session::session_owed(&state) {
        let _ = app.emit("session:owed", true);
        crate::kiosk::engage(&app, &state);
    }
    Ok(())
}

#[tauri::command]
pub fn get_app_state(state: State<'_, AppState>) -> CmdResult<AppStateView> {
    let (onboarded, hour, minute) = {
        let conn = state.db.0.lock().unwrap();
        (
            matches!(db::get_config(&conn, "onboarded"), Ok(Some(v)) if v == "1"),
            db::get_config(&conn, "schedule_hour")
                .ok()
                .flatten()
                .and_then(|v| v.parse().ok())
                .unwrap_or(9),
            db::get_config(&conn, "schedule_minute")
                .ok()
                .flatten()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        )
    };
    let enforcement_disarmed = std::env::var_os("HOME")
        .map(|h| std::path::Path::new(&h).join("sdr-unlock").exists())
        .unwrap_or(false);
    let (schedule_paused, kiosk_level, selected_focus) = {
        let conn = state.db.0.lock().unwrap();
        let today = state.today();
        (
            matches!(db::get_config(&conn, "schedule_paused"), Ok(Some(v)) if v == "1"),
            db::get_config(&conn, "kiosk_level")
                .ok()
                .flatten()
                .unwrap_or_else(|| "hard".into()),
            session::session_focus(&conn, &today)
                .ok()
                .flatten()
                .or_else(|| {
                    crate::mastery::get_profile(&conn, "preferred_focus")
                        .ok()
                        .flatten()
                        .filter(|focus| crate::focus::is_selectable(focus))
                }),
        )
    };
    let (classroom_programs, classroom_slots, active_classroom_sessions) = {
        let conn = state.db.0.lock().unwrap();
        (
            crate::classroom::program_views(&conn, &state.today()).map_err(err)?,
            crate::classroom::slot_views(&conn, &state.today(), state.debug_day).map_err(err)?,
            crate::classroom::active_sessions(&conn).map_err(err)?,
        )
    };
    let classroom_due_count = classroom_slots.iter().filter(|slot| slot.owed).count();
    Ok(AppStateView {
        onboarded,
        session: session::view(&state),
        owed: session::session_owed(&state),
        schedule_hour: hour,
        schedule_minute: minute,
        agent_ok: None,
        debug_day: state.debug_day,
        enforcement_disarmed,
        schedule_paused,
        kiosk_level,
        model: state.generator.current_model(),
        agent: state.generator.current_agent(),
        custom_agent_bin: state.generator.current_custom_bin(),
        selected_focus,
        deepseek_key_configured: deepseek_key_configured(),
        classroom_programs,
        classroom_slots,
        classroom_due_count,
        active_classroom_sessions,
    })
}

fn refresh_os_schedule(state: &AppState) -> CmdResult<()> {
    if state.debug_day {
        return Ok(());
    }
    let times = {
        let conn = state.db.0.lock().unwrap();
        if matches!(db::get_config(&conn, "schedule_paused"), Ok(Some(value)) if value == "1") {
            return Ok(());
        }
        crate::classroom::all_schedule_times(&conn).map_err(err)?
    };
    crate::scheduler::install_many(&times)
}

fn deepseek_key_configured() -> bool {
    std::env::var("DEEPSEEK_API_KEY")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .is_some()
        || crate::keychain::has_secret("deepseek")
}

/// Store (or clear, with an empty string) the DeepSeek API key. Persisted
/// in the macOS Keychain, not the app database — see `keychain.rs`.
#[tauri::command]
pub fn set_deepseek_api_key(key: String) -> CmdResult<()> {
    crate::keychain::set_secret("deepseek", &key)
}

/// Pause the daily schedule entirely: launchd agent removed, owed checks
/// disabled, countdown hidden — dormant until resume_schedule.
#[tauri::command]
pub fn pause_schedule(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    {
        let conn = state.db.0.lock().unwrap();
        db::set_config(&conn, "schedule_paused", "1").map_err(err)?;
    }
    if !state.debug_day {
        crate::scheduler::uninstall()?;
    }
    let _ = app.emit("session:state", session::view(&state));
    Ok(())
}

#[tauri::command]
pub fn resume_schedule(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    {
        let conn = state.db.0.lock().unwrap();
        db::set_config(&conn, "schedule_paused", "0").map_err(err)?;
    }
    refresh_os_schedule(&state)?;
    let _ = app.emit("session:state", session::view(&state));
    Ok(())
}

/// Ping whichever agent is currently selected (optionally an explicit one,
/// so the setup wizard can test a choice before saving it).
#[tauri::command]
pub async fn check_agent(
    state: State<'_, AppState>,
    agent: Option<String>,
    custom_bin: Option<String>,
) -> CmdResult<bool> {
    Ok(agents::test_connection(&state, agent, custom_bin, None)
        .await?
        .ok)
}

#[derive(Deserialize)]
pub struct SetupInput {
    pub hour: u32,
    pub minute: u32,
    pub escape_phrase: String,
    #[serde(default)]
    pub kiosk_level: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub custom_agent_bin: Option<String>,
}

fn validate_schedule_time(hour: u32, minute: u32) -> CmdResult<()> {
    if hour > 23 || minute > 59 {
        Err(format!(
            "invalid schedule time {hour:02}:{minute:02}; hour must be 0-23 and minute 0-59"
        ))
    } else {
        Ok(())
    }
}

#[tauri::command]
pub async fn complete_setup(
    app: AppHandle,
    state: State<'_, AppState>,
    input: SetupInput,
) -> CmdResult<AppStateView> {
    validate_schedule_time(input.hour, input.minute)?;
    if input.escape_phrase.trim().len() < 40 {
        return Err("escape phrase must be at least 40 characters".into());
    }
    {
        let conn = state.db.0.lock().unwrap();
        db::set_config(&conn, "schedule_hour", &input.hour.to_string()).map_err(err)?;
        db::set_config(&conn, "schedule_minute", &input.minute.to_string()).map_err(err)?;
        db::set_config(&conn, "escape_phrase", input.escape_phrase.trim()).map_err(err)?;
        let level = input.kiosk_level.as_deref().unwrap_or("hard");
        if !valid_kiosk_level(level) {
            return Err(format!("unknown kiosk level: {level}"));
        }
        db::set_config(&conn, "kiosk_level", level).map_err(err)?;
        let model = input.model.as_deref().unwrap_or("opus");
        if !valid_model(model) {
            return Err(format!("unknown model: {model}"));
        }
        db::set_config(&conn, "model", model).map_err(err)?;
        *state.generator.model.lock().unwrap() = model.to_string();
        let agent = input.agent.as_deref().unwrap_or("claude");
        if !valid_agent(agent) {
            return Err(format!("unknown agent: {agent}"));
        }
        let custom = input.custom_agent_bin.clone().unwrap_or_default();
        if agent == "custom" && custom.trim().is_empty() {
            return Err("custom agent needs a binary path".into());
        }
        db::set_config(&conn, "agent", agent).map_err(err)?;
        db::set_config(&conn, "custom_agent_bin", custom.trim()).map_err(err)?;
        *state.generator.agent.lock().unwrap() = agent.to_string();
        *state.generator.custom_bin.lock().unwrap() = custom.trim().to_string();
        db::set_config(&conn, "onboarded", "1").map_err(err)?;
        // Day-1 content generates after the student picks a focus at session start.
    }
    refresh_os_schedule(&state)?;
    state.gen_notify.notify_one();
    let _ = app.emit("session:state", session::view(&state));
    get_app_state(state)
}

#[tauri::command]
pub fn update_schedule(state: State<'_, AppState>, hour: u32, minute: u32) -> CmdResult<()> {
    validate_schedule_time(hour, minute)?;
    {
        let conn = state.db.0.lock().unwrap();
        db::set_config(&conn, "schedule_hour", &hour.to_string()).map_err(err)?;
        db::set_config(&conn, "schedule_minute", &minute.to_string()).map_err(err)?;
    }
    refresh_os_schedule(&state)
}

#[tauri::command]
pub fn get_curriculum_map(
    state: State<'_, AppState>,
    focus: String,
) -> CmdResult<crate::classroom::CurriculumMapView> {
    let conn = state.db.0.lock().unwrap();
    crate::classroom::curriculum_map(&conn, &focus).map_err(err)
}

#[tauri::command]
pub fn configure_classroom_program(
    app: AppHandle,
    state: State<'_, AppState>,
    input: crate::classroom::ConfigureClassroomInput,
) -> CmdResult<crate::classroom::ClassroomProgramView> {
    {
        let conn = state.db.0.lock().unwrap();
        crate::classroom::configure_program(&conn, &input, &state.today()).map_err(err)?;
    }
    refresh_os_schedule(&state)?;
    let view = {
        let conn = state.db.0.lock().unwrap();
        crate::classroom::program_view(&conn, &input.subject_id, &state.today()).map_err(err)?
    };
    let _ = app.emit("classroom:state", &view);
    Ok(view)
}

#[tauri::command]
pub fn upsert_classroom_slot(
    app: AppHandle,
    state: State<'_, AppState>,
    input: crate::classroom::UpsertClassroomSlotInput,
) -> CmdResult<Vec<crate::classroom::ClassroomSlotView>> {
    {
        let conn = state.db.0.lock().unwrap();
        crate::classroom::upsert_slot(&conn, &input).map_err(err)?;
    }
    refresh_os_schedule(&state)?;
    let slots = {
        let conn = state.db.0.lock().unwrap();
        crate::classroom::slot_views(&conn, &state.today(), state.debug_day).map_err(err)?
    };
    let _ = app.emit("classroom:state", &slots);
    Ok(slots)
}

#[tauri::command]
pub fn delete_classroom_slot(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> CmdResult<Vec<crate::classroom::ClassroomSlotView>> {
    {
        let conn = state.db.0.lock().unwrap();
        crate::classroom::delete_slot(&conn, id).map_err(err)?;
    }
    refresh_os_schedule(&state)?;
    let slots = {
        let conn = state.db.0.lock().unwrap();
        crate::classroom::slot_views(&conn, &state.today(), state.debug_day).map_err(err)?
    };
    let _ = app.emit("classroom:state", &slots);
    Ok(slots)
}

/// Preview (`commit: false`) or persist (`commit: true`) a schedule derived
/// from a learner's stated goal, weekly-minutes target, and availability
/// windows. Additive to `upsert_classroom_slot` — committing only replaces
/// this subject's previously *planned* slots, never hand-placed ones.
#[tauri::command]
pub fn plan_classroom_schedule(
    app: AppHandle,
    state: State<'_, AppState>,
    input: crate::classroom::PlanClassroomScheduleInput,
) -> CmdResult<crate::classroom::ClassroomPlanView> {
    let commit = input.commit;
    let plan = {
        let conn = state.db.0.lock().unwrap();
        crate::classroom::plan_schedule(&conn, &input, &state.today()).map_err(err)?
    };
    if commit {
        refresh_os_schedule(&state)?;
        let _ = app.emit("classroom:state", &plan);
    }
    Ok(plan)
}

/// Route a classroom start to the subject's isolated engine. Advisory classes
/// never engage the kiosk or mutate the primary frontend session row.
/// `revisit` is the opt-in path for re-serving a completed module.
#[tauri::command]
pub async fn start_classroom_session(
    app: AppHandle,
    state: State<'_, AppState>,
    subject_id: String,
    slot_id: Option<i64>,
    revisit: Option<bool>,
) -> CmdResult<serde_json::Value> {
    if session::session_owed(&state) {
        return Err(
            "The focused daily study session is due. Complete or skip it before starting a classroom class."
                .into(),
        );
    }
    let revisit = revisit.unwrap_or(false);
    let spec = crate::classroom::subject(subject_id.trim()).map_err(err)?;
    let value = match spec.kind {
        crate::classroom::SubjectKind::Language => {
            let (base, seed, program) = {
                let conn = state.db.0.lock().unwrap();
                let base = crate::language::start_classroom_session(
                    &conn,
                    spec.id,
                    slot_id,
                    &state.today(),
                    revisit,
                )
                .map_err(err)?;
                let seed = crate::language::stored_lesson(&conn, base.session_id).map_err(err)?;
                let program = crate::classroom::program_row(&conn, spec.id).map_err(err)?;
                (base, seed, program)
            };
            let profile = crate::classroom::generation_profile(&program);
            let contract = crate::classroom::contract_with_goal(&program, spec.prompt);
            let (generated, _) = state
                .generator
                .enrich_classroom_language_lesson(&contract, &profile, &seed)
                .await;
            let lesson = {
                let conn = state.db.0.lock().unwrap();
                crate::language::replace_stored_lesson(&conn, base.session_id, &generated)
                    .map_err(err)?
            };
            serde_json::json!({ "kind": "language", "lesson": lesson })
        }
        crate::classroom::SubjectKind::Engineering => {
            let lesson =
                crate::classroom::start_engineering_session(&state, spec.id, slot_id, revisit)
                    .await?;
            serde_json::json!({ "kind": "engineering", "lesson": lesson })
        }
    };
    let _ = app.emit("classroom:state", &value);
    Ok(value)
}

#[tauri::command]
pub fn resume_classroom_session(
    state: State<'_, AppState>,
    subject_id: String,
) -> CmdResult<Option<serde_json::Value>> {
    if session::session_owed(&state) {
        return Err(
            "The focused daily study session is due. Complete or skip it before resuming a classroom class."
                .into(),
        );
    }
    let spec = crate::classroom::subject(subject_id.trim()).map_err(err)?;
    match spec.kind {
        crate::classroom::SubjectKind::Language => {
            let conn = state.db.0.lock().unwrap();
            Ok(crate::language::active_session_for(&conn, spec.id)
                .map_err(err)?
                .map(|lesson| serde_json::json!({ "kind": "language", "lesson": lesson })))
        }
        crate::classroom::SubjectKind::Engineering => {
            let conn = state.db.0.lock().unwrap();
            Ok(crate::classroom::active_engineering_session(&conn, spec.id)
                .map_err(err)?
                .map(|lesson| serde_json::json!({ "kind": "engineering", "lesson": lesson })))
        }
    }
}

#[tauri::command]
pub fn submit_classroom_engineering_session(
    app: AppHandle,
    state: State<'_, AppState>,
    input: crate::classroom::SubmitEngineeringInput,
) -> CmdResult<crate::classroom::EngineeringSessionResult> {
    let result = {
        let conn = state.db.0.lock().unwrap();
        crate::classroom::submit_engineering_session(&conn, &input, &state.today()).map_err(err)?
    };
    let _ = app.emit("classroom:state", &result);
    Ok(result)
}

#[tauri::command]
pub fn submit_language_session(
    app: AppHandle,
    state: State<'_, AppState>,
    input: crate::language::SubmitSessionInput,
) -> CmdResult<crate::language::LanguageSessionResult> {
    let result = {
        let conn = state.db.0.lock().unwrap();
        crate::language::submit_session(&conn, &input, &state.today()).map_err(err)?
    };
    let _ = app.emit("classroom:state", &result);
    Ok(result)
}

#[tauri::command]
pub async fn start_session(
    app: AppHandle,
    state: State<'_, AppState>,
    focus: String,
) -> CmdResult<SessionView> {
    let s = session::start_session(&state, Some(focus.trim()))
        .await
        .map_err(err)?;
    {
        let conn = state.db.0.lock().unwrap();
        let _ = crate::mastery::set_profile(&conn, "preferred_focus", &s.focus);
    }
    if s.status == "in_progress" {
        crate::kiosk::engage(&app, &state);
    }
    let v = session::view(&state);
    let _ = app.emit("session:state", v.clone());
    Ok(v)
}

#[derive(Serialize, Deserialize, Clone)]
pub struct QuizQuestionView {
    pub id: i64,
    pub prompt: String,
    pub kind: String,
    pub choices: Option<Vec<String>>,
    pub origin: String,
    pub answered: bool,
    pub draft: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct QuizRoundView {
    pub round_id: Option<RoundId>,
    pub revision: u32,
    pub questions: Vec<QuizQuestionView>,
}

#[tauri::command]
pub fn get_quiz(state: State<'_, AppState>) -> CmdResult<QuizRoundView> {
    let today = state.today();
    let yesterday = state.yesterday();
    let conn = state.db.0.lock().unwrap();
    let questions = session::questions_for_today(&conn, &today, &yesterday).map_err(err)?;
    let round = primary_quiz::current(&conn, &today).map_err(err)?;
    let views = questions
        .into_iter()
        .map(|q| {
            let response = round
                .as_ref()
                .and_then(|r| r.responses.get(&q.id.to_string()));
            QuizQuestionView {
                id: q.id,
                prompt: q.prompt,
                kind: q.kind,
                choices: q
                    .choices_json
                    .as_deref()
                    .and_then(|c| serde_json::from_str(c).ok()),
                origin: q.origin,
                answered: response.is_some_and(|r| r.status == ResponseStatus::Answered),
                draft: response.map(|r| r.answer.clone()),
            }
        })
        .collect();
    Ok(QuizRoundView {
        round_id: round.as_ref().map(|r| r.id.clone()),
        revision: round.map_or(0, |r| r.revision),
        questions: views,
    })
}

#[tauri::command]
pub fn submit_answer(
    state: State<'_, AppState>,
    round_id: RoundId,
    expected_revision: u32,
    question_id: i64,
    answer: String,
    confirmed: bool,
) -> CmdResult<u32> {
    let conn = state.db.0.lock().unwrap();
    Ok(primary_quiz::save_answer(
        &conn,
        &state.today(),
        &round_id,
        expected_revision,
        question_id,
        answer,
        confirmed,
    )
    .map_err(err)?
    .revision)
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ReviewItem {
    pub question_id: i64,
    pub prompt: String,
    pub kind: String,
    pub user_answer: String,
    pub correct: Option<bool>,
    pub feedback: String,
    pub correct_answer: String,
    pub explanation: String,
    pub returns_tomorrow: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ReviewData {
    pub items: Vec<ReviewItem>,
    pub score: f64,
    pub self_assess: bool,
}

#[tauri::command]
pub async fn finish_quiz(
    app: AppHandle,
    state: State<'_, AppState>,
    round_id: RoundId,
    expected_revision: u32,
) -> CmdResult<ReviewData> {
    let today = state.today();
    let tomorrow = state.tomorrow();

    let (round, questions, pending, concept_of, track_focus, dossier) = {
        let conn = state.db.0.lock().unwrap();
        let round = primary_quiz::checked_round(&conn, &today, &round_id).map_err(err)?;
        if let Some(submission) = &round.submission {
            return serde_json::from_value(submission.result.clone()).map_err(err);
        }
        if round.revision != expected_revision {
            return Err("answers changed; reload the saved round before submitting".into());
        }
        let current = db::get_session(&conn, &today)
            .map_err(err)?
            .ok_or("no session")?;
        if current.status != "in_progress" || current.current_step != session::STEP_QUIZ {
            return Err("this session is not awaiting quiz answers".into());
        }
        let track_focus = session::session_focus(&conn, &today)
            .map_err(err)?
            .ok_or("session focus not chosen")?;
        let qs = primary_quiz::questions(&round).map_err(err)?;
        let pending = primary_quiz::pending(&round);
        if qs.iter().any(|q| {
            round
                .responses
                .get(&q.id.to_string())
                .is_none_or(|response| {
                    response.status != ResponseStatus::Answered || response.answer.trim().is_empty()
                })
        }) {
            return Err("answer every question before submitting".into());
        }
        // question_id -> (concept_id, concept_title), for grading context + mastery.
        let mut stmt = conn
            .prepare(
                "SELECT q.id, c.id, c.title FROM questions q
                 JOIN courses co ON co.id = q.course_id
                 JOIN concepts c ON c.id = co.concept_id",
            )
            .map_err(err)?;
        let concept_of: HashMap<i64, (i64, String)> = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    (r.get::<_, i64>(1)?, r.get::<_, String>(2)?),
                ))
            })
            .map_err(err)?
            .filter_map(|r| r.ok())
            .collect();
        let dossier =
            crate::mastery::build_dossier(&conn, &today, &track_focus).unwrap_or_default();
        (round, qs, pending, concept_of, track_focus, dossier)
    };

    // Grade free-text via agent in one batch.
    let free_items: Vec<GradeItem> = questions
        .iter()
        .filter(|q| q.kind == "free")
        .map(|q| GradeItem {
            id: q.id,
            concept: concept_of
                .get(&q.id)
                .map(|(_, t)| t.clone())
                .unwrap_or_default(),
            question: q.prompt.clone(),
            model_answer: q.correct_answer.clone(),
            user_answer: pending.get(&q.id).cloned().unwrap_or_default(),
        })
        .collect();
    let verdicts = state
        .generator
        .grade(&free_items, &dossier, &track_focus)
        .await;
    let self_assess = verdicts.is_none();
    let verdict_map: HashMap<i64, (bool, String, String)> = verdicts
        .unwrap_or_default()
        .into_iter()
        .map(|v| (v.id, (v.correct, v.feedback, v.note)))
        .collect();

    let result = {
        let conn = state.db.0.lock().unwrap();
        persist_quiz_result(
            &conn,
            &today,
            &tomorrow,
            &round,
            &questions,
            &pending,
            &concept_of,
            &verdict_map,
            self_assess,
        )?
    };
    let _ = app.emit("session:state", ());
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn persist_quiz_result(
    conn: &rusqlite::Connection,
    today: &str,
    tomorrow: &str,
    expected: &Round,
    questions: &[db::Question],
    pending: &HashMap<i64, String>,
    concept_of: &HashMap<i64, (i64, String)>,
    verdict_map: &HashMap<i64, (bool, String, String)>,
    self_assess: bool,
) -> CmdResult<ReviewData> {
    let mut items = Vec::new();
    let mut n_graded = 0usize;
    let mut n_correct = 0usize;
    // concept_id -> (correct, total) for mastery transitions after the loop.
    let mut by_concept: HashMap<i64, (usize, usize)> = HashMap::new();
    {
        let transaction = conn.unchecked_transaction().map_err(err)?;
        let conn = transaction;
        let frozen = primary_quiz::checked_round(&conn, today, &expected.id).map_err(err)?;
        if let Some(submission) = &frozen.submission {
            return serde_json::from_value(submission.result.clone()).map_err(err);
        }
        if frozen.revision != expected.revision
            || frozen.responses != expected.responses
            || serde_json::to_value(questions).map_err(err)?
                != serde_json::to_value(primary_quiz::questions(expected).map_err(err)?)
                    .map_err(err)?
        {
            return Err("answers changed while grading; submit the saved answers again".into());
        }
        let current = db::get_session(&conn, today)
            .map_err(err)?
            .ok_or("no session")?;
        if current.status != "in_progress" || current.current_step != session::STEP_QUIZ {
            return Err(
                "the session changed while grading; your answers were not submitted".into(),
            );
        }
        if &primary_quiz::pending(&frozen) != pending {
            return Err("answers changed while grading; submit the updated answers again".into());
        }
        for q in questions {
            let user_answer = pending.get(&q.id).cloned().unwrap_or_default();
            let (correct, feedback): (Option<bool>, String) = if q.kind == "mcq" {
                let ok = user_answer.trim() == q.correct_answer.trim();
                (Some(ok), String::new())
            } else if let Some((ok, fb, _note)) = verdict_map.get(&q.id) {
                (Some(*ok), fb.clone())
            } else {
                (
                    None,
                    "grader unavailable — self-assess against the model answer".into(),
                )
            };
            let counted_correct = correct.unwrap_or(true); // self-assess counts as pass, never blocks
            if correct.is_some() {
                n_graded += 1;
                if counted_correct {
                    n_correct += 1;
                }
                if let Some((cid, _)) = concept_of.get(&q.id) {
                    let e = by_concept.entry(*cid).or_insert((0, 0));
                    e.1 += 1;
                    if counted_correct {
                        e.0 += 1;
                    }
                }
            }
            // Teacher's per-concept observation from the grader, kept for next encounter.
            if let Some((_, _, note)) = verdict_map.get(&q.id) {
                if let Some((cid, _)) = concept_of.get(&q.id) {
                    crate::mastery::set_teacher_note(&conn, *cid, note).map_err(err)?;
                }
            }
            db::record_attempt(
                &conn,
                &Attempt {
                    question_id: q.id,
                    session_date: today.to_string(),
                    user_answer: user_answer.clone(),
                    correct: counted_correct,
                    grader_feedback: feedback.clone(),
                },
            )
            .map_err(err)?;
            let failed = correct == Some(false);
            if failed {
                db::push_carryover(&conn, q.id, today, tomorrow).map_err(err)?;
            } else if q.origin == "carryover" && correct == Some(true) {
                db::clear_carryover(&conn, q.id).map_err(err)?;
            }
            items.push(ReviewItem {
                question_id: q.id,
                prompt: q.prompt.clone(),
                kind: q.kind.clone(),
                user_answer,
                correct,
                feedback,
                correct_answer: q.correct_answer.clone(),
                explanation: q.explanation.clone(),
                returns_tomorrow: failed,
            });
        }
        let score = if n_graded > 0 {
            n_correct as f64 / n_graded as f64
        } else {
            1.0
        };
        // Mastery transitions: one quiz encounter per concept touched today.
        for (cid, (ok, total)) in &by_concept {
            if *total > 0 {
                crate::mastery::record_quiz_outcome(&conn, *cid, today, *ok as f64 / *total as f64)
                    .map_err(err)?;
            }
        }
        let mut s = db::get_session(&conn, today)
            .map_err(err)?
            .ok_or("no session")?;
        s.quiz_score = Some(score);
        s.current_step = session::STEP_REVIEW.into();
        db::upsert_session(&conn, &s).map_err(err)?;
        let result = ReviewData {
            items,
            score,
            self_assess,
        };
        assessments::submit_in_transaction(
            &conn,
            &Owner::LegacyPrimary(today.into()),
            expected,
            &serde_json::to_value(&result).map_err(err)?,
            true,
        )
        .map_err(err)?;
        conn.commit().map_err(err)?;
        Ok(result)
    }
}

#[tauri::command]
pub fn get_review(state: State<'_, AppState>) -> CmdResult<ReviewData> {
    let today = state.today();
    let yesterday = state.yesterday();
    let conn = state.db.0.lock().unwrap();
    if let Some(result) = primary_quiz::result(&conn, &today).map_err(err)? {
        return serde_json::from_value(result).map_err(err);
    }
    let attempts = db::attempts_for_session(&conn, &today).map_err(err)?;
    let mut by_q: HashMap<i64, &Attempt> = HashMap::new();
    for a in &attempts {
        by_q.insert(a.question_id, a);
    }
    // Reconstruct from questions answered today (carryover rows may already be cleared).
    let mut items = Vec::new();
    let mut n_graded = 0usize;
    let mut n_correct = 0usize;
    let mut self_assess = false;
    let mut stmt = conn
        .prepare(
            "SELECT q.id, q.prompt, q.kind, q.correct_answer, q.explanation, q.origin
             FROM questions q JOIN attempts a ON a.question_id = q.id
             WHERE a.session_date = ?1 ORDER BY a.id",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([&today], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })
        .map_err(err)?;
    for row in rows {
        let (id, prompt, kind, correct_answer, explanation, _origin) = row.map_err(err)?;
        if let Some(a) = by_q.get(&id) {
            let ungraded = a
                .grader_feedback
                .starts_with("grader unavailable — self-assess");
            if ungraded {
                self_assess = true;
            } else {
                n_graded += 1;
                if a.correct {
                    n_correct += 1;
                }
            }
            items.push(ReviewItem {
                question_id: id,
                prompt,
                kind,
                user_answer: a.user_answer.clone(),
                correct: if ungraded { None } else { Some(a.correct) },
                feedback: a.grader_feedback.clone(),
                correct_answer,
                explanation,
                returns_tomorrow: !ungraded && !a.correct,
            });
        }
    }
    let score = if n_graded == 0 {
        1.0
    } else {
        n_correct as f64 / n_graded as f64
    };
    let _ = yesterday;
    Ok(ReviewData {
        items,
        score,
        self_assess,
    })
}

#[derive(Serialize)]
pub struct RouletteView {
    pub pool: Vec<String>,
    pub chosen_index: usize,
    pub concept_title: String,
    pub concept_category: String,
    pub pool_unlocked: usize,
    pub pool_total: usize,
    /// Every module in the track is completed: no new lesson can be drawn.
    /// The wheel is empty and the frontend offers a track-complete state
    /// (with an opt-in revisit instead of an automatic re-serve).
    pub track_complete: bool,
}

/// End a lesson day whose track has no drawable module left. Only legal from
/// the roulette step and only when the track is actually complete — the
/// mastered track's retrieval lives in pop-quiz days, not repeated lessons.
#[tauri::command]
pub fn complete_track_day(app: AppHandle, state: State<'_, AppState>) -> CmdResult<SessionView> {
    let today = state.today();
    {
        let conn = state.db.0.lock().unwrap();
        let track_focus = session::session_focus(&conn, &today)
            .map_err(err)?
            .ok_or("session focus not chosen")?;
        if crate::roulette::drawable_exists(&conn, &track_focus).map_err(err)? {
            return Err("the track still has new modules — draw one instead".into());
        }
    }
    session::complete_session(&app, &state).map_err(err)?;
    let v = session::view(&state);
    let _ = app.emit("session:state", v.clone());
    Ok(v)
}

#[tauri::command]
pub fn finish_review(app: AppHandle, state: State<'_, AppState>) -> CmdResult<SessionView> {
    let is_pop = {
        let today = state.today();
        let conn = state.db.0.lock().unwrap();
        db::get_session(&conn, &today)
            .map_err(err)?
            .map(|s| s.session_type == "pop_quiz")
            .unwrap_or(false)
    };
    if is_pop {
        // Pop-quiz day: the audit IS the session — no new topic, done after review.
        session::complete_session(&app, &state).map_err(err)?;
    } else {
        session::set_step(&state, session::STEP_ROULETTE).map_err(err)?;
    }
    let v = session::view(&state);
    let _ = app.emit("session:state", v.clone());
    Ok(v)
}

/// Wheel data: pool of titles + the index of today's (pre-decided) concept.
/// `revisit` is the opt-in path that draws from completed modules only —
/// never the automatic selection.
#[tauri::command]
pub fn get_roulette(state: State<'_, AppState>, revisit: Option<bool>) -> CmdResult<RouletteView> {
    let today = state.today();
    let conn = state.db.0.lock().unwrap();
    let track_focus = session::session_focus(&conn, &today)
        .map_err(err)?
        .ok_or("session focus not chosen")?;
    let empty_view = RouletteView {
        pool: vec![],
        chosen_index: 0,
        concept_title: String::new(),
        concept_category: String::new(),
        pool_unlocked: 0,
        pool_total: 0,
        track_complete: true,
    };
    // Reuse pre-drawn concept if pregen already picked one, else draw now.
    let concept = {
        let existing = db::get_session(&conn, &today)
            .map_err(err)?
            .and_then(|s| s.concept_id);
        match existing {
            Some(id) => db::get_concept(&conn, id)
                .map_err(err)?
                .ok_or("concept missing")?,
            None => {
                let drawn = if revisit.unwrap_or(false) {
                    crate::roulette::draw_completed(&conn, &today, &track_focus).map_err(err)?
                } else {
                    crate::roulette::draw(&conn, &today, &track_focus).map_err(err)?
                };
                let Some(c) = drawn else {
                    let completed_available =
                        crate::roulette::draw_completed(&conn, &today, &track_focus)
                            .map_err(err)?
                            .is_some();
                    if !revisit.unwrap_or(false) && completed_available {
                        // Track mastered: every unlocked module is completed.
                        return Ok(empty_view);
                    }
                    return Err(if revisit.unwrap_or(false) {
                        "no completed modules to revisit".into()
                    } else {
                        "empty pool".into()
                    });
                };
                let mut s = db::get_session(&conn, &today)
                    .map_err(err)?
                    .ok_or("no session")?;
                s.concept_id = Some(c.id);
                db::upsert_session(&conn, &s).map_err(err)?;
                c
            }
        }
    };
    // Build a wheel pool from UNLOCKED, uncompleted concepts only:
    // chosen + up to 11 others. Completed modules never appear in the wheel.
    let (_, locked) = crate::roulette::pool_status(&conn, &track_focus).map_err(err)?;
    let drawable = crate::roulette::drawable(&conn, &track_focus).map_err(err)?;
    let pool_unlocked = drawable.len();
    let pool_total = pool_unlocked + locked.len();
    let mut pool: Vec<String> = drawable
        .into_iter()
        .filter(|c| c.id != concept.id)
        .take(11)
        .map(|c| c.title)
        .collect();
    let chosen_index = (chrono::Utc::now().timestamp() as usize) % (pool.len() + 1);
    pool.insert(chosen_index.min(pool.len()), concept.title.clone());
    Ok(RouletteView {
        pool,
        chosen_index,
        concept_title: concept.title,
        concept_category: concept.category,
        pool_unlocked,
        pool_total,
        track_complete: false,
    })
}

#[derive(Serialize)]
pub struct CourseView {
    pub course_id: i64,
    pub title: String,
    pub concept_slug: String,
    pub curriculum: db::CurriculumBrief,
    pub prerequisites: Vec<String>,
    pub session_index: i64,
    pub why_now: String,
    pub markdown: String,
    pub resources: serde_json::Value,
    pub source: String,
    pub remaining_seconds: i64,
    pub total_seconds: i64,
}

/// Generate today's course if missing (slow path), then return it. Frontend shows
/// gen:status events while this runs.
#[tauri::command]
pub async fn ensure_course(app: AppHandle, state: State<'_, AppState>) -> CmdResult<CourseView> {
    let today = state.today();
    let _ = app.emit("gen:status", "checking course");
    let course = session::ensure_course_for_date(&state, &today)
        .await
        .map_err(|e| e.to_string())?;
    let (concept, prerequisites, session_index) = {
        let conn = state.db.0.lock().unwrap();
        let concept = db::get_concept(&conn, course.concept_id)
            .map_err(err)?
            .ok_or("course concept no longer exists")?;
        let prerequisites_json: String = conn
            .query_row(
                "SELECT prereqs_json FROM concepts WHERE id = ?1",
                [concept.id],
                |row| row.get(0),
            )
            .map_err(err)?;
        let session_index: i64 = conn
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM sessions
                     WHERE status = 'completed' AND focus = ?1)
                  + (SELECT COUNT(*) FROM classroom_sessions
                     WHERE status = 'completed' AND subject_id = ?1) + 1",
                [&concept.focus],
                |row| row.get(0),
            )
            .map_err(err)?;
        (
            concept,
            serde_json::from_str(&prerequisites_json).unwrap_or_default(),
            session_index,
        )
    };
    let total = state.course_duration_secs();
    let s = {
        let conn = state.db.0.lock().unwrap();
        db::get_session(&conn, &today)
            .map_err(err)?
            .ok_or("no session")?
    };
    let remaining = (total - s.reading_seconds).max(0);
    Ok(CourseView {
        course_id: course.id,
        title: concept.title.clone(),
        concept_slug: concept.slug,
        prerequisites,
        session_index,
        why_now: format!(
            "Session {session_index} advances the {} phase: {}",
            concept.curriculum.phase, concept.curriculum.learner_outcome
        ),
        curriculum: concept.curriculum,
        markdown: course.markdown,
        resources: serde_json::from_str(&course.resources_json).unwrap_or(serde_json::json!([])),
        source: course.source,
        remaining_seconds: remaining,
        total_seconds: total,
    })
}

#[tauri::command]
pub fn start_course(app: AppHandle, state: State<'_, AppState>) -> CmdResult<SessionView> {
    let today = state.today();
    let total = state.course_duration_secs();
    let remaining = {
        let conn = state.db.0.lock().unwrap();
        let s = db::get_session(&conn, &today)
            .map_err(err)?
            .ok_or("no session")?;
        (total - s.reading_seconds).max(0)
    };
    session::set_step(&state, session::STEP_COURSE).map_err(err)?;
    state.reading_remaining.store(remaining, Ordering::SeqCst);
    if !state.timer_running.load(Ordering::SeqCst) && remaining > 0 {
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            session::run_course_timer(app2).await;
        });
    }
    let v = session::view(&state);
    let _ = app.emit("session:state", v.clone());
    Ok(v)
}

#[tauri::command]
pub fn finish_course(app: AppHandle, state: State<'_, AppState>) -> CmdResult<SessionView> {
    let remaining = state.reading_remaining.load(Ordering::SeqCst);
    if remaining > 0 {
        return Err(format!("{remaining} seconds of reading remain"));
    }
    session::complete_session(&app, &state).map_err(err)?;
    Ok(session::view(&state))
}

/// Audio lesson for today: returns the dialogue script (generating it live if
/// missing — ~1 min sonnet call) plus the playback engine. The frontend uses
/// speechSynthesis unless rendered files exist.
#[tauri::command]
pub async fn ensure_audio(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<crate::audio::AudioView> {
    let today = state.today();
    let course = {
        let conn = state.db.0.lock().unwrap();
        db::course_for_date(&conn, &today)
            .map_err(err)?
            .ok_or("no course today")?
    };
    let _ = app.emit("gen:status", "writing audio script from today's course");
    session::ensure_audio_for_course(&state, &course, &today)
        .await
        .map_err(|e| format!("audio unavailable: {e}"))?;
    let conn = state.db.0.lock().unwrap();
    crate::audio::get_script(&conn, course.id)
        .map_err(err)?
        .ok_or_else(|| "audio script missing after generation".into())
}

#[tauri::command]
pub fn get_audio_enabled(state: State<'_, AppState>) -> CmdResult<bool> {
    let conn = state.db.0.lock().unwrap();
    Ok(matches!(db::get_config(&conn, "audio_enabled"), Ok(Some(v)) if v == "1"))
}

#[tauri::command]
pub fn set_audio_enabled(state: State<'_, AppState>, enabled: bool) -> CmdResult<()> {
    let conn = state.db.0.lock().unwrap();
    db::set_config(&conn, "audio_enabled", if enabled { "1" } else { "0" }).map_err(err)
}

#[derive(Serialize)]
pub struct ExitQuizQuestion {
    pub id: i64,
    pub prompt: String,
    pub choices: Vec<String>,
}

const INITIAL_EXIT_QUESTION_COUNT: usize = 5;

fn exit_round_key(course_id: i64) -> String {
    format!("exit_quiz_round:{course_id}")
}

fn exit_count_key(course_id: i64) -> String {
    format!("exit_quiz_count:{course_id}")
}

/// Compact record of exactly what the student missed in the round that
/// just failed — read back by `get_exit_quiz` to target the next round at
/// those learning objectives instead of resampling the whole course.
fn exit_failed_areas_key(course_id: i64) -> String {
    format!("exit_failed_areas:{course_id}")
}

fn read_failed_areas(
    conn: &rusqlite::Connection,
    course_id: i64,
) -> Vec<crate::generator::FailedArea> {
    db::get_config(conn, &exit_failed_areas_key(course_id))
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn write_failed_areas(
    conn: &rusqlite::Connection,
    course_id: i64,
    areas: &[crate::generator::FailedArea],
) -> CmdResult<()> {
    let json = serde_json::to_string(areas).map_err(err)?;
    db::set_config(conn, &exit_failed_areas_key(course_id), &json).map_err(err)
}

fn exit_progress(conn: &rusqlite::Connection, course_id: i64) -> (i64, usize) {
    let round = db::get_config(conn, &exit_round_key(course_id))
        .ok()
        .flatten()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1);
    let count = db::get_config(conn, &exit_count_key(course_id))
        .ok()
        .flatten()
        .and_then(|value| value.parse().ok())
        .unwrap_or(INITIAL_EXIT_QUESTION_COUNT);
    // No floor at INITIAL_EXIT_QUESTION_COUNT here: `get_exit_quiz` may
    // have persisted a smaller, deliberately-shrunk count when a short
    // course's bundled pool couldn't cover a full round, and that choice
    // must stick — re-flooring it back up would recreate the exact
    // impossible-to-fill round the shrink exists to avoid.
    (round.max(1), count.max(1))
}

/// A generated exit-check item is usable as long as it has a real prompt,
/// at least two choices, and a correct answer that actually appears among
/// them once whitespace is trimmed. The model is asked for exactly 4
/// choices, but rejecting anything that drifts from that (rather than
/// tolerating minor formatting noise) is what used to starve rounds of
/// otherwise-valid questions and hard-fail the exit check.
fn is_usable_exit_check(check: &crate::generator::ExitCheck) -> bool {
    let correct = check.correct_answer.trim();
    !check.prompt.trim().is_empty()
        && !correct.is_empty()
        && check.choices.len() >= 2
        && check.choices.iter().any(|choice| choice.trim() == correct)
}

/// Pull filler MCQs from the bundled offline pool for `focus` when the
/// live agent can't fill out a round after retrying — the exit check must
/// never block a session the way the rest of generation never does. Draws
/// from every bundled course for the focus, not just one, since a single
/// course rarely has enough spare MCQs by itself.
///
/// When `failed_areas` is non-empty (a remediation round), bundled
/// questions whose section/objective/prompt loosely match a missed area
/// are preferred first — "matching bundled questions where possible" per
/// the remediation contract — before falling back to the unfiltered pool
/// so a round is still always fillable.
fn fallback_exit_checks(
    focus: &str,
    needed: usize,
    seen: &mut std::collections::HashSet<String>,
    failed_areas: &[crate::generator::FailedArea],
) -> Vec<crate::generator::ExitCheck> {
    let to_check = |q: crate::generator::GeneratedQuestion| crate::generator::ExitCheck {
        prompt: q.prompt,
        choices: q.choices.unwrap_or_default(),
        correct_answer: q.correct_answer,
        explanation: q.explanation,
        section: q.section,
        learning_objective: q.learning_objective,
    };
    let pool = crate::generator::fallback_mcq_pool(focus);
    let keywords: Vec<String> = failed_areas
        .iter()
        .flat_map(|a| {
            [
                a.section.to_lowercase(),
                a.learning_objective.to_lowercase(),
            ]
        })
        .flat_map(|s| s.split_whitespace().map(str::to_string).collect::<Vec<_>>())
        .filter(|w| w.len() > 3)
        .collect();
    let matches_failed_area = |q: &crate::generator::GeneratedQuestion| {
        if keywords.is_empty() {
            return false;
        }
        let haystack = format!(
            "{} {} {}",
            q.section.to_lowercase(),
            q.learning_objective.to_lowercase(),
            q.prompt.to_lowercase()
        );
        keywords.iter().any(|kw| haystack.contains(kw.as_str()))
    };

    let mut out = Vec::new();
    if !keywords.is_empty() {
        out.extend(
            pool.iter()
                .filter(|q| matches_failed_area(q))
                .cloned()
                .filter_map(|q| {
                    let normalized = q.prompt.trim().to_lowercase();
                    if normalized.is_empty() || !seen.insert(normalized) {
                        return None;
                    }
                    Some(to_check(q))
                })
                .filter(is_usable_exit_check)
                .take(needed),
        );
    }
    if out.len() < needed {
        let remaining = needed - out.len();
        out.extend(
            pool.into_iter()
                .filter_map(|q| {
                    let normalized = q.prompt.trim().to_lowercase();
                    if normalized.is_empty() || !seen.insert(normalized) {
                        return None;
                    }
                    Some(to_check(q))
                })
                .filter(is_usable_exit_check)
                .take(remaining),
        );
    }
    out
}

/// Return the current adaptive exit-check round. Missing questions are
/// generated on demand and never repeat a prompt from an earlier round.
/// Generation trouble (missing API key, malformed model output, an
/// unreachable agent) degrades to bundled filler questions rather than
/// blocking the reader — mirrors the never-blocks guarantee course and
/// quiz generation already give the rest of the session.
#[tauri::command]
pub async fn get_exit_quiz(state: State<'_, AppState>) -> CmdResult<Vec<ExitQuizQuestion>> {
    let today = state.today();
    let (course, focus, round, target_count, mut exclusions, failed_areas) = {
        let conn = state.db.0.lock().unwrap();
        let course = db::course_for_date(&conn, &today)
            .map_err(err)?
            .ok_or("no course today")?;
        let focus = db::get_concept(&conn, course.concept_id)
            .map_err(err)?
            .map(|concept| concept.focus)
            .ok_or("course concept missing")?;
        let (round, target_count) = exit_progress(&conn, course.id);
        let exclusions = db::all_exit_question_prompts(&conn, course.id).map_err(err)?;
        let failed_areas = if round > 1 {
            read_failed_areas(&conn, course.id)
        } else {
            Vec::new()
        };
        (course, focus, round, target_count, exclusions, failed_areas)
    };

    let mut existing = {
        let conn = state.db.0.lock().unwrap();
        db::exit_questions_for_course(&conn, course.id, round).map_err(err)?
    };
    let mut generation_attempts = 0;
    while existing.len() < target_count && generation_attempts < 2 {
        generation_attempts += 1;
        let needed = target_count - existing.len();
        let generated = match state
            .generator
            .generate_exit_quiz(&course.markdown, &focus, needed, &exclusions, &failed_areas)
            .await
        {
            Ok(checks) => checks,
            Err(e) => {
                log::warn!("exit check generation attempt failed, will retry or use filler: {e}");
                Vec::new()
            }
        };
        let conn = state.db.0.lock().unwrap();
        let mut seen: std::collections::HashSet<String> = exclusions
            .iter()
            .map(|prompt| prompt.trim().to_lowercase())
            .collect();
        let mut inserted = 0;
        for q in generated {
            if inserted >= needed {
                break;
            }
            let normalized = q.prompt.trim().to_lowercase();
            if normalized.is_empty() || !seen.insert(normalized) || !is_usable_exit_check(&q) {
                continue;
            }
            db::insert_exit_question(
                &conn,
                course.id,
                round,
                &q.prompt,
                &serde_json::to_string(&q.choices).map_err(err)?,
                &q.correct_answer,
                &q.explanation,
                &q.section,
                &q.learning_objective,
            )
            .map_err(err)?;
            exclusions.push(q.prompt);
            inserted += 1;
        }
        existing = db::exit_questions_for_course(&conn, course.id, round).map_err(err)?;
    }

    if existing.len() < target_count {
        let mut seen: std::collections::HashSet<String> = exclusions
            .iter()
            .map(|prompt| prompt.trim().to_lowercase())
            .collect();
        let needed = target_count - existing.len();
        let filler = fallback_exit_checks(&focus, needed, &mut seen, &failed_areas);
        if !filler.is_empty() {
            let conn = state.db.0.lock().unwrap();
            for q in filler {
                db::insert_exit_question(
                    &conn,
                    course.id,
                    round,
                    &q.prompt,
                    &serde_json::to_string(&q.choices).map_err(err)?,
                    &q.correct_answer,
                    &q.explanation,
                    &q.section,
                    &q.learning_objective,
                )
                .map_err(err)?;
            }
            existing = db::exit_questions_for_course(&conn, course.id, round).map_err(err)?;
        }
    }
    // Both the live agent and the entire bundled pool for this focus came
    // up short of fresh material. Rather than block the reader
    // indefinitely (there may simply not be `target_count` more distinct
    // questions to ask about a short course), grade against whatever
    // could actually be assembled — shrinking the round, never erroring,
    // as long as there is at least one usable question.
    let effective_count = if existing.len() < target_count {
        if existing.is_empty() {
            return Err(
                "no exit-check questions are available yet for this course — keep reading a \
                 little longer, then try again"
                    .into(),
            );
        }
        let conn = state.db.0.lock().unwrap();
        db::set_config(
            &conn,
            &exit_count_key(course.id),
            &existing.len().to_string(),
        )
        .map_err(err)?;
        existing.len()
    } else {
        target_count
    };

    Ok(existing
        .into_iter()
        .take(effective_count)
        .map(|q| ExitQuizQuestion {
            id: q.id,
            prompt: q.prompt,
            choices: q.choices,
        })
        .collect())
}

#[derive(Serialize)]
pub struct ExitQuizFeedback {
    pub question_id: i64,
    pub prompt: String,
    pub user_answer: String,
    pub correct_answer: String,
    pub explanation: String,
    pub section: String,
    pub learning_objective: String,
}

#[derive(Serialize)]
pub struct ExitQuizResult {
    pub passed: bool,
    pub correct: Vec<i64>,
    pub incorrect: Vec<ExitQuizFeedback>,
    pub round: i64,
    pub next_question_count: usize,
    /// Human-readable labels of what the next round will target — empty
    /// when passed, or when a miss carried no section/objective metadata.
    pub next_focus_areas: Vec<String>,
}

fn grade_exit_round(
    questions: &[db::ExitQuestion],
    answers: &HashMap<i64, String>,
) -> CmdResult<(Vec<i64>, Vec<ExitQuizFeedback>)> {
    if answers.len() != questions.len()
        || questions
            .iter()
            .any(|question| !answers.contains_key(&question.id))
        || answers
            .keys()
            .any(|id| !questions.iter().any(|question| question.id == *id))
    {
        return Err("answer every question in the current exit-check round".into());
    }

    let mut correct = Vec::new();
    let mut incorrect = Vec::new();
    for question in questions {
        let user_answer = answers.get(&question.id).cloned().unwrap_or_default();
        if user_answer.trim() == question.correct_answer.trim() {
            correct.push(question.id);
        } else {
            incorrect.push(ExitQuizFeedback {
                question_id: question.id,
                prompt: question.prompt.clone(),
                user_answer,
                correct_answer: question.correct_answer.clone(),
                explanation: question.explanation.clone(),
                section: question.section.clone(),
                learning_objective: question.learning_objective.clone(),
            });
        }
    }
    Ok((correct, incorrect))
}

/// Grade immediately. A perfect round burns the remaining TTL; every miss is
/// explained, recorded for remediation, and increases the size of the next
/// fresh round by one.
#[tauri::command]
pub fn submit_exit_quiz(
    app: AppHandle,
    state: State<'_, AppState>,
    answers: HashMap<i64, String>,
) -> CmdResult<ExitQuizResult> {
    let today = state.today();
    let (course_id, round, question_count, questions) = {
        let conn = state.db.0.lock().unwrap();
        let course = db::course_for_date(&conn, &today)
            .map_err(err)?
            .ok_or("no course today")?;
        let (round, question_count) = exit_progress(&conn, course.id);
        let mut questions = db::exit_questions_for_course(&conn, course.id, round).map_err(err)?;
        questions.truncate(question_count);
        (course.id, round, question_count, questions)
    };
    if questions.len() != question_count {
        return Err("no exit check exists for today".into());
    }
    let (correct, incorrect) = grade_exit_round(&questions, &answers)?;
    let passed = incorrect.is_empty();
    let next_question_count = if passed {
        question_count
    } else {
        question_count + incorrect.len()
    };
    let next_focus_areas: Vec<String> = incorrect
        .iter()
        .map(|f| {
            crate::generator::FailedArea {
                section: f.section.clone(),
                learning_objective: f.learning_objective.clone(),
            }
            .label()
        })
        .collect();
    {
        // Durable audit trail — every graded answer this round, correct or not.
        let conn = state.db.0.lock().unwrap();
        for id in &correct {
            if let Some(q) = questions.iter().find(|q| q.id == *id) {
                let _ = db::insert_exit_attempt(
                    &conn,
                    course_id,
                    q.id,
                    round,
                    true,
                    answers.get(&q.id).map(|s| s.as_str()).unwrap_or(""),
                    &q.section,
                    &q.learning_objective,
                    "",
                );
            }
        }
        for f in &incorrect {
            let _ = db::insert_exit_attempt(
                &conn,
                course_id,
                f.question_id,
                round,
                false,
                &f.user_answer,
                &f.section,
                &f.learning_objective,
                &f.explanation,
            );
        }
    }
    if passed {
        // Burn the remaining TTL: the running timer sees 0 and fires timer:done.
        let total = state.course_duration_secs();
        state.reading_remaining.store(0, Ordering::SeqCst);
        let conn = state.db.0.lock().unwrap();
        if let Ok(Some(mut s)) = db::get_session(&conn, &today) {
            s.reading_seconds = total;
            let _ = db::upsert_session(&conn, &s);
        }
        let _ = app.emit("timer:tick", 0);
        let _ = app.emit("timer:done", true);
    } else {
        let conn = state.db.0.lock().unwrap();
        db::set_config(&conn, &exit_round_key(course_id), &(round + 1).to_string()).map_err(err)?;
        db::set_config(
            &conn,
            &exit_count_key(course_id),
            &next_question_count.to_string(),
        )
        .map_err(err)?;
        // Next round's targeting: only these exact missed objectives, deduped.
        let mut dedup_seen = std::collections::HashSet::new();
        let areas: Vec<crate::generator::FailedArea> = incorrect
            .iter()
            .map(|f| crate::generator::FailedArea {
                section: f.section.clone(),
                learning_objective: f.learning_objective.clone(),
            })
            .filter(|a| dedup_seen.insert(a.clone()))
            .collect();
        write_failed_areas(&conn, course_id, &areas)?;
    }
    Ok(ExitQuizResult {
        passed,
        correct,
        incorrect,
        round,
        next_question_count,
        next_focus_areas,
    })
}

#[tauri::command]
pub fn escape_session(
    app: AppHandle,
    state: State<'_, AppState>,
    phrase: String,
) -> CmdResult<bool> {
    match crate::kiosk::verify_escape(&state, &phrase)? {
        true => {
            let today = state.today();
            {
                let conn = state.db.0.lock().unwrap();
                let focus = crate::mastery::get_profile(&conn, "preferred_focus")
                    .ok()
                    .flatten()
                    .filter(|value| crate::focus::is_selectable(value))
                    .unwrap_or_else(|| "javascript".into());
                let mut s = db::get_session(&conn, &today)
                    .ok()
                    .flatten()
                    .unwrap_or(db::Session {
                        date: today.clone(),
                        concept_id: None,
                        status: "pending".into(),
                        current_step: session::STEP_QUIZ.into(),
                        quiz_score: None,
                        started_at: None,
                        completed_at: None,
                        reading_seconds: 0,
                        session_type: "lesson".into(),
                        plan_reason: "emergency skip before session start".into(),
                        focus,
                    });
                s.status = "skipped".into();
                s.completed_at = Some(session::now_iso());
                let _ = db::upsert_session(&conn, &s);
            }
            state.clear_chat_threads();
            crate::kiosk::release(&app, &state);
            let _ = app.emit("session:state", session::view(&state));
            Ok(true)
        }
        false => Ok(false),
    }
}

#[tauri::command]
pub fn get_escape_phrase(state: State<'_, AppState>) -> CmdResult<String> {
    let conn = state.db.0.lock().unwrap();
    Ok(db::get_config(&conn, "escape_phrase")
        .map_err(err)?
        .unwrap_or_default())
}

#[derive(Serialize)]
pub struct DashboardView {
    pub history: Vec<db::HistoryEntry>,
    pub streak: i64,
    pub carryover_due: i64,
    pub concepts_total: i64,
    pub concepts_covered: i64,
    pub mastery: Vec<crate::mastery::MasteryEntry>,
}

#[tauri::command]
pub fn get_dashboard(state: State<'_, AppState>) -> CmdResult<DashboardView> {
    let today = state.today();
    let tomorrow = state.tomorrow();
    let conn = state.db.0.lock().unwrap();
    let track_focus = session::session_focus(&conn, &today)
        .map_err(err)?
        .unwrap_or_else(|| "javascript".to_string());
    let history = db::history(&conn, 120).map_err(err)?;
    let streak = db::streak(&conn, &today).map_err(err)?;
    let carryover_due = db::carryover_count(&conn, &tomorrow, &track_focus).map_err(err)?;
    let concepts_total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM concepts WHERE active = 1 AND focus = ?1",
            params![track_focus],
            |r| r.get(0),
        )
        .map_err(err)?;
    let concepts_covered: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM concepts WHERE times_picked > 0 AND focus = ?1",
            params![track_focus],
            |r| r.get(0),
        )
        .map_err(err)?;
    let mastery = crate::mastery::overview(&conn, &track_focus).map_err(err)?;
    Ok(DashboardView {
        history,
        streak,
        carryover_due,
        concepts_total,
        concepts_covered,
        mastery,
    })
}

#[derive(Serialize)]
pub struct ArchivedCourse {
    pub course_id: i64,
    pub session_date: String,
    pub title: String,
    pub markdown: String,
    pub resources: serde_json::Value,
}

#[tauri::command]
pub fn get_past_course(
    state: State<'_, AppState>,
    date: String,
) -> CmdResult<Option<ArchivedCourse>> {
    let conn = state.db.0.lock().unwrap();
    let Some(course) = db::course_for_date(&conn, &date).map_err(err)? else {
        return Ok(None);
    };
    let title = db::get_concept(&conn, course.concept_id)
        .map_err(err)?
        .map(|c| c.title)
        .unwrap_or_default();
    Ok(Some(ArchivedCourse {
        course_id: course.id,
        session_date: course.session_date,
        title,
        markdown: course.markdown,
        resources: serde_json::from_str(&course.resources_json).unwrap_or(serde_json::json!([])),
    }))
}

/// One exercise workspace view for either owner: a primary course or a
/// classroom session (exactly one of the two ids must be present).
#[tauri::command]
pub fn get_exercise(
    state: State<'_, AppState>,
    course_id: Option<i64>,
    classroom_session_id: Option<i64>,
) -> CmdResult<Option<db::ExerciseView>> {
    let conn = state.db.0.lock().unwrap();
    if let Some(id) = classroom_session_id {
        return crate::classroom::classroom_exercise(&conn, id).map_err(err);
    }
    let Some(id) = course_id else {
        return Err("exercise owner must be a course or a classroom session".into());
    };
    let Some(exercise) = db::get_course_exercise(&conn, id).map_err(err)? else {
        return Ok(None);
    };
    let draft = db::get_exercise_draft(&conn, Some(id), None).map_err(err)?;
    let (completed, reflection) =
        db::get_exercise_completion(&conn, Some(id), None).map_err(err)?;
    Ok(Some(db::ExerciseView {
        course_id: Some(exercise.course_id),
        classroom_session_id: None,
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

/// Debounced on the frontend; the backend just persists whatever draft
/// text it is given. Never touches the session timer, kiosk lock, or
/// mastery/completion state.
#[tauri::command]
pub fn save_exercise_draft(
    state: State<'_, AppState>,
    course_id: Option<i64>,
    classroom_session_id: Option<i64>,
    draft: String,
) -> CmdResult<()> {
    let conn = state.db.0.lock().unwrap();
    db::save_exercise_draft(&conn, course_id, classroom_session_id, &draft).map_err(err)
}

/// Self-certified practice evidence. Completion stays non-blocking, but the
/// reflection enters the learner dossier so future lessons can build on work
/// the student actually attempted rather than assuming every exercise was done.
#[tauri::command]
pub fn save_exercise_completion(
    state: State<'_, AppState>,
    course_id: Option<i64>,
    classroom_session_id: Option<i64>,
    completed: bool,
    reflection: String,
) -> CmdResult<()> {
    if completed && reflection.split_whitespace().count() < 5 {
        return Err(
            "add a short evidence/trade-off reflection before marking this complete".into(),
        );
    }
    let conn = state.db.0.lock().unwrap();
    db::save_exercise_completion(
        &conn,
        course_id,
        classroom_session_id,
        completed,
        &reflection,
    )
    .map_err(err)
}

#[derive(Serialize, Clone)]
pub struct ChatMessageView {
    pub role: String,
    pub content: String,
    pub section: Option<String>,
    pub follow_ups: Vec<String>,
}

fn chat_view(turns: &[crate::generator::ChatTurn]) -> Vec<ChatMessageView> {
    turns
        .iter()
        .map(|t| ChatMessageView {
            role: t.role.clone(),
            content: t.content.clone(),
            section: t.section.clone(),
            follow_ups: t.follow_ups.clone(),
        })
        .collect()
}

const MAX_CHAT_MESSAGE_CHARS: usize = 2_000;

fn prepare_chat_message(message: String) -> CmdResult<String> {
    let message = message.trim().to_string();
    if message.is_empty() {
        return Err("message cannot be empty".into());
    }
    if message.chars().count() > MAX_CHAT_MESSAGE_CHARS {
        return Err(format!(
            "message is too long; keep it under {MAX_CHAT_MESSAGE_CHARS} characters"
        ));
    }
    Ok(message)
}

/// Current in-memory chat thread for one owner: a primary course or a
/// classroom session (exactly one id). Empty (never an error) when nothing
/// has been asked yet — the whole thread lives only for the active app
/// session and is gone on restart, completion, or skip.
#[tauri::command]
pub fn get_chat(
    state: State<'_, AppState>,
    course_id: Option<i64>,
    classroom_session_id: Option<i64>,
) -> CmdResult<Vec<ChatMessageView>> {
    let key = chat_key(course_id, classroom_session_id)?;
    let threads = state.chat_threads.lock().unwrap();
    Ok(threads
        .get(&key)
        .map(|turns| chat_view(turns))
        .unwrap_or_default())
}

fn chat_key(course_id: Option<i64>, classroom_session_id: Option<i64>) -> CmdResult<String> {
    match (course_id, classroom_session_id) {
        (Some(id), None) => Ok(format!("course:{id}")),
        (None, Some(id)) => Ok(format!("classroom:{id}")),
        _ => Err("chat owner must be exactly one of course or classroom session".into()),
    }
}

/// Ask one bounded, course-grounded question. Never touches the reading
/// timer, kiosk lock, mastery, or completion state — purely a session-only
/// side conversation about the course already on screen.
#[tauri::command]
pub async fn send_chat_message(
    state: State<'_, AppState>,
    course_id: Option<i64>,
    classroom_session_id: Option<i64>,
    message: String,
) -> CmdResult<Vec<ChatMessageView>> {
    let key = chat_key(course_id, classroom_session_id)?;
    let message = prepare_chat_message(message)?;
    let history = state
        .chat_threads
        .lock()
        .unwrap()
        .get(&key)
        .cloned()
        .unwrap_or_default();
    let reply = if let Some(id) = classroom_session_id {
        let (context, profile) = {
            let conn = state.db.0.lock().unwrap();
            let context = crate::classroom::engineering_chat_context(&conn, id).map_err(err)?;
            let program = crate::classroom::program_row(&conn, &context.focus).map_err(err)?;
            let profile = crate::classroom::generation_profile(&program);
            (context, profile)
        };
        state
            .generator
            .answer_course_question_for(&context, &message, &history, &profile)
            .await
            .map_err(err)?
    } else {
        let course_id = course_id.ok_or("chat owner must be a course")?;
        let context = {
            let conn = state.db.0.lock().unwrap();
            let course = db::get_course(&conn, course_id)
                .map_err(err)?
                .ok_or("course not found")?;
            let concept = db::get_concept(&conn, course.concept_id)
                .map_err(err)?
                .ok_or("course concept not found")?;
            let exercise = db::get_course_exercise(&conn, course_id)
                .map_err(err)?
                .map(|value| serde_json::to_string_pretty(&value))
                .transpose()
                .map_err(err)?
                .unwrap_or_else(|| "(this course has no separate exercise)".into());
            crate::generator::CourseChatContext {
                title: concept.title,
                focus: concept.focus,
                markdown: course.markdown,
                learner_outcome: concept.curriculum.learner_outcome,
                cumulative_artifact: concept.curriculum.artifact,
                exercise,
            }
        };
        state
            .generator
            .answer_course_question(&context, &message, &history)
            .await
            .map_err(err)?
    };
    let mut threads = state.chat_threads.lock().unwrap();
    let thread = threads.entry(key).or_default();
    thread.push(crate::generator::ChatTurn::user(message));
    thread.push(crate::generator::ChatTurn::assistant(reply));
    Ok(chat_view(thread))
}

/// After the session, open all of today's resource links in the default browser.
#[tauri::command]
pub fn open_resources(app: AppHandle, state: State<'_, AppState>) -> CmdResult<usize> {
    if state.locked.load(Ordering::SeqCst) {
        return Err("locked — resources unlock after the session".into());
    }
    let today = state.today();
    let urls: Vec<String> = {
        let conn = state.db.0.lock().unwrap();
        let course = db::course_for_date(&conn, &today).map_err(err)?;
        course
            .map(|c| {
                serde_json::from_str::<Vec<crate::generator::Resource>>(&c.resources_json)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|r| r.url)
                    .collect()
            })
            .unwrap_or_default()
    };
    let n = urls.len();
    for url in urls {
        use tauri_plugin_opener::OpenerExt;
        let _ = app.opener().open_url(url, None::<String>);
    }
    Ok(n)
}

#[cfg(test)]
mod exit_quiz_tests {
    use super::*;

    #[test]
    fn schedule_time_validation_rejects_out_of_range_values() {
        assert!(validate_schedule_time(23, 59).is_ok());
        assert!(validate_schedule_time(24, 0).is_err());
        assert!(validate_schedule_time(8, 60).is_err());
    }

    fn question(id: i64, correct_answer: &str, explanation: &str) -> db::ExitQuestion {
        db::ExitQuestion {
            id,
            prompt: format!("Question {id}"),
            choices: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            correct_answer: correct_answer.into(),
            explanation: explanation.into(),
            section: String::new(),
            learning_objective: String::new(),
        }
    }

    #[test]
    fn grading_returns_explanations_for_each_miss() {
        let questions = vec![
            question(1, "a", "A is correct"),
            question(2, "b", "B is correct"),
        ];
        let answers = HashMap::from([(1, "a".to_string()), (2, "c".to_string())]);

        let (correct, incorrect) = grade_exit_round(&questions, &answers).unwrap();

        assert_eq!(correct, vec![1]);
        assert_eq!(incorrect.len(), 1);
        assert_eq!(incorrect[0].user_answer, "c");
        assert_eq!(incorrect[0].correct_answer, "b");
        assert_eq!(incorrect[0].explanation, "B is correct");
    }

    #[test]
    fn grading_rejects_incomplete_rounds() {
        let questions = vec![
            question(1, "a", "A is correct"),
            question(2, "b", "B is correct"),
        ];
        let answers = HashMap::from([(1, "a".to_string())]);

        assert!(grade_exit_round(&questions, &answers).is_err());
    }

    fn check(prompt: &str, choices: &[&str], correct: &str) -> crate::generator::ExitCheck {
        crate::generator::ExitCheck {
            prompt: prompt.into(),
            choices: choices.iter().map(|c| c.to_string()).collect(),
            correct_answer: correct.into(),
            explanation: "because".into(),
            section: String::new(),
            learning_objective: String::new(),
        }
    }

    #[test]
    fn usable_exit_check_accepts_four_clean_choices() {
        assert!(is_usable_exit_check(&check(
            "Q",
            &["a", "b", "c", "d"],
            "b"
        )));
    }

    #[test]
    fn usable_exit_check_tolerates_whitespace_drift_in_correct_answer() {
        // Regression: the model padding "b " while choices has "b" used to
        // silently drop an otherwise-valid question and starve the round.
        assert!(is_usable_exit_check(&check(
            "Q",
            &["a", "b", "c", "d"],
            "b "
        )));
    }

    #[test]
    fn usable_exit_check_tolerates_non_four_choice_counts() {
        // Regression: requiring exactly 4 choices rejected valid 3- or
        // 5-option MCQs the model returned, even though the correct answer
        // is unambiguous.
        assert!(is_usable_exit_check(&check("Q", &["a", "b", "c"], "c")));
    }

    #[test]
    fn usable_exit_check_rejects_missing_correct_answer() {
        assert!(!is_usable_exit_check(&check(
            "Q",
            &["a", "b", "c", "d"],
            "z"
        )));
    }

    #[test]
    fn usable_exit_check_rejects_empty_prompt_or_single_choice() {
        assert!(!is_usable_exit_check(&check("", &["a", "b"], "a")));
        assert!(!is_usable_exit_check(&check("Q", &["a"], "a")));
    }

    static TEST_DB_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn test_conn() -> rusqlite::Connection {
        let n = TEST_DB_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("sdr-cmd-test-{}-{}", std::process::id(), n));
        std::fs::create_dir_all(&dir).unwrap();
        db::open(&dir.join("test.db")).unwrap()
    }

    #[test]
    fn exit_progress_honors_a_deliberately_shrunk_count() {
        // Regression: `count.max(INITIAL_EXIT_QUESTION_COUNT)` used to
        // re-floor a round that `get_exit_quiz` had shrunk because the
        // bundled pool ran out of fresh MCQs, recreating the exact
        // impossible-to-fill round the shrink exists to avoid.
        let conn = test_conn();
        db::set_config(&conn, &exit_count_key(1), "2").unwrap();
        let (_, count) = exit_progress(&conn, 1);
        assert_eq!(count, 2);
    }

    #[test]
    fn exit_progress_defaults_to_initial_count_when_unset() {
        let conn = test_conn();
        let (round, count) = exit_progress(&conn, 42);
        assert_eq!(round, 1);
        assert_eq!(count, INITIAL_EXIT_QUESTION_COUNT);
    }

    #[test]
    fn fallback_exit_checks_skip_already_seen_prompts_and_respect_needed() {
        let mut seen = std::collections::HashSet::new();
        let filler = fallback_exit_checks("javascript", 2, &mut seen, &[]);
        assert!(filler.len() <= 2);
        assert!(filler.iter().all(is_usable_exit_check));

        // Asking again with the same `seen` set must not repeat a prompt —
        // this is exactly what keeps a retried round from looping forever.
        let more = fallback_exit_checks("javascript", 2, &mut seen, &[]);
        for q in &more {
            assert!(!filler.iter().any(|f| f.prompt == q.prompt));
        }
    }

    #[test]
    fn fallback_exit_checks_prefer_matching_failed_areas_when_present() {
        let mut seen = std::collections::HashSet::new();
        let failed_areas = vec![crate::generator::FailedArea {
            section: "Core mechanics".into(),
            learning_objective: "microtasks drain before the next macrotask".into(),
        }];
        let filler = fallback_exit_checks("javascript", 1, &mut seen, &failed_areas);
        // The bundled js-event-loop course has a matching MCQ for this
        // objective; a real match should win over an arbitrary pool pick.
        assert!(!filler.is_empty());
        let picked = &filler[0];
        assert!(
            picked.learning_objective.to_lowercase().contains("microtask")
                || picked.section.to_lowercase().contains("mechanic"),
            "expected the targeted pick to actually match a failed area, got section={:?} objective={:?}",
            picked.section,
            picked.learning_objective
        );
    }

    #[test]
    fn fallback_exit_checks_degrades_gracefully_when_pool_is_smaller_than_needed() {
        // Regression: provider failure must never trap the reader. Asking
        // for far more than the bundled pool can supply must return
        // whatever is available (deduped, usable) instead of panicking or
        // looping — the caller shrinks the round around whatever comes back.
        let mut seen = std::collections::HashSet::new();
        let pool_size = crate::generator::fallback_mcq_pool("javascript").len();
        let filler = fallback_exit_checks("javascript", pool_size + 50, &mut seen, &[]);
        assert!(filler.len() <= pool_size);
        assert!(filler.iter().all(is_usable_exit_check));
        let mut prompts: Vec<&str> = filler.iter().map(|f| f.prompt.as_str()).collect();
        let unique_count = {
            prompts.sort_unstable();
            prompts.dedup();
            prompts.len()
        };
        assert_eq!(unique_count, filler.len(), "filler must not repeat prompts");
    }

    #[test]
    fn submitting_a_failed_round_persists_targeted_failed_areas() {
        let conn = test_conn();
        db::seed_concepts(&conn, crate::SEED_CONCEPTS).unwrap();
        let course_id =
            db::insert_course(&conn, "2026-01-01", 1, "# course", "[]", "fallback").unwrap();
        db::insert_exit_question(
            &conn,
            course_id,
            1,
            "Q1",
            &serde_json::to_string(&vec!["a", "b"]).unwrap(),
            "a",
            "explain",
            "Core mechanics",
            "objective one",
        )
        .unwrap();
        let questions = db::exit_questions_for_course(&conn, course_id, 1).unwrap();
        let answers = HashMap::from([(questions[0].id, "b".to_string())]);
        let (_, incorrect) = grade_exit_round(&questions, &answers).unwrap();
        assert_eq!(incorrect.len(), 1);

        let areas = vec![crate::generator::FailedArea {
            section: incorrect[0].section.clone(),
            learning_objective: incorrect[0].learning_objective.clone(),
        }];
        write_failed_areas(&conn, course_id, &areas).unwrap();
        let read_back = read_failed_areas(&conn, course_id);
        assert_eq!(read_back.len(), 1);
        assert_eq!(read_back[0].learning_objective, "objective one");
    }

    #[test]
    fn chat_view_preserves_grounding_and_follow_ups() {
        let turns = vec![
            crate::generator::ChatTurn::user("What blocks the loop?".into()),
            crate::generator::ChatTurn::assistant(crate::generator::ChatReply {
                answer: "Only synchronous CPU work on the stack.".into(),
                section: "The precise model".into(),
                follow_ups: vec![
                    "Can you trace one blocking task?".into(),
                    "How would you measure the delay?".into(),
                    "Where does the exercise expose it?".into(),
                ],
            }),
        ];
        let view = chat_view(&turns);
        assert_eq!(view.len(), 2);
        assert_eq!(view[0].role, "user");
        assert_eq!(view[0].content, "What blocks the loop?");
        assert_eq!(view[1].role, "assistant");
        assert_eq!(view[1].content, "Only synchronous CPU work on the stack.");
        assert_eq!(view[1].section.as_deref(), Some("The precise model"));
        assert_eq!(view[1].follow_ups.len(), 3);
    }

    #[test]
    fn chat_message_validation_trims_and_caps_input() {
        assert_eq!(
            prepare_chat_message("  explain this trace  ".into()).unwrap(),
            "explain this trace"
        );
        assert!(prepare_chat_message(" \n ".into()).is_err());
        assert!(prepare_chat_message("x".repeat(MAX_CHAT_MESSAGE_CHARS + 1)).is_err());
    }
}

#[cfg(test)]
mod quiz_persistence_tests {
    use super::*;

    struct Fixture {
        conn: rusqlite::Connection,
        questions: Vec<db::Question>,
        round: Round,
        answers: HashMap<i64, String>,
        concepts: HashMap<i64, (i64, String)>,
    }

    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "principia-quiz-{}-{}.db",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let conn = db::open(&path).unwrap();
            db::seed_concepts(&conn, include_str!("../seed/concepts.json")).unwrap();
            let concept = db::all_concepts(&conn, "javascript").unwrap()[0].clone();
            let course = db::insert_course(
                &conn,
                "2026-07-20",
                concept.id,
                "# Prior lesson",
                "[]",
                "fallback",
            )
            .unwrap();
            let id = db::insert_question(
                &conn,
                course,
                "What happens?",
                "mcq",
                Some(r#"["A","B"]"#),
                "A",
                "A follows the mechanism.",
            )
            .unwrap();
            db::upsert_session(
                &conn,
                &db::Session {
                    date: "2026-07-21".into(),
                    concept_id: None,
                    status: "in_progress".into(),
                    current_step: "quiz".into(),
                    quiz_score: None,
                    started_at: None,
                    completed_at: None,
                    reading_seconds: 0,
                    session_type: "lesson".into(),
                    plan_reason: String::new(),
                    focus: "javascript".into(),
                },
            )
            .unwrap();
            let answers = HashMap::from([(id, "A".into())]);
            let questions = db::questions_for_course(&conn, course).unwrap();
            let round = primary_quiz::freeze(&conn, "2026-07-21", &questions).unwrap();
            let round = primary_quiz::save_answer(
                &conn,
                "2026-07-21",
                &round.id,
                round.revision,
                id,
                "A".into(),
                true,
            )
            .unwrap();
            Self {
                conn,
                questions,
                round,
                answers,
                concepts: HashMap::from([(id, (concept.id, concept.title))]),
            }
        }

        fn submit(&self) -> CmdResult<ReviewData> {
            persist_quiz_result(
                &self.conn,
                "2026-07-21",
                "2026-07-22",
                &self.round,
                &self.questions,
                &self.answers,
                &self.concepts,
                &HashMap::new(),
                false,
            )
        }
    }

    #[test]
    fn retry_returns_the_original_result_without_duplicate_assessment_evidence() {
        let fixture = Fixture::new();
        let first = fixture.submit().unwrap();
        let retry = fixture.submit().unwrap();
        assert_eq!(
            serde_json::to_value(first).unwrap(),
            serde_json::to_value(retry).unwrap()
        );
        assert_eq!(
            db::attempts_for_session(&fixture.conn, "2026-07-21")
                .unwrap()
                .len(),
            1
        );
        let concept_id = fixture.concepts.values().next().unwrap().0;
        assert_eq!(
            crate::mastery::get(&fixture.conn, concept_id)
                .unwrap()
                .encounters,
            1
        );
    }

    #[test]
    fn late_storage_failure_rolls_back_attempts_mastery_and_session_changes() {
        let fixture = Fixture::new();
        fixture.conn.execute_batch("CREATE TRIGGER fail_quiz_commit BEFORE UPDATE OF quiz_score ON sessions BEGIN SELECT RAISE(ABORT, 'injected storage failure'); END;").unwrap();
        assert!(fixture
            .submit()
            .err()
            .unwrap()
            .contains("injected storage failure"));
        assert!(db::attempts_for_session(&fixture.conn, "2026-07-21")
            .unwrap()
            .is_empty());
        let concept_id = fixture.concepts.values().next().unwrap().0;
        assert_eq!(
            crate::mastery::get(&fixture.conn, concept_id)
                .unwrap()
                .encounters,
            0
        );
        assert_eq!(
            db::get_session(&fixture.conn, "2026-07-21")
                .unwrap()
                .unwrap()
                .current_step,
            "quiz"
        );
        assert_eq!(
            primary_quiz::pending(
                &primary_quiz::current(&fixture.conn, "2026-07-21")
                    .unwrap()
                    .unwrap()
            ),
            fixture.answers
        );
        fixture
            .conn
            .execute_batch("DROP TRIGGER fail_quiz_commit;")
            .unwrap();
        assert_eq!(fixture.submit().unwrap().score, 1.0);
    }

    #[test]
    fn changing_answers_during_grading_keeps_the_new_draft_and_rejects_the_stale_result() {
        let fixture = Fixture::new();
        primary_quiz::save_answer(
            &fixture.conn,
            "2026-07-21",
            &fixture.round.id,
            fixture.round.revision,
            fixture.questions[0].id,
            "B".into(),
            true,
        )
        .unwrap();
        assert!(fixture.submit().err().unwrap().contains("answers changed"));
        assert!(db::attempts_for_session(&fixture.conn, "2026-07-21")
            .unwrap()
            .is_empty());
    }
}

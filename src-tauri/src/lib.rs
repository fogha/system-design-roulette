pub mod agents;
pub mod audio;
pub mod catalog;
pub mod classroom;
pub mod commands;
pub mod db;
pub mod domain;
pub mod focus;
pub mod generator;
pub mod keychain;
pub mod kiosk;
pub mod language;
pub mod mastery;
pub mod progress;
pub mod research;
pub mod roulette;
pub mod scheduler;
pub mod session;
pub mod state;
pub mod storage;
pub mod subjects;

use state::AppState;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Mutex;
use tauri::{Emitter, Manager};

const SEED_CONCEPTS: &str = include_str!("../seed/concepts.json");

fn resolve_claude_bin() -> String {
    if let Ok(p) = std::env::var("SDR_CLAUDE_BIN") {
        return p;
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let candidates = [
        format!("{home}/.local/bin/claude"),
        "/usr/local/bin/claude".to_string(),
        "/opt/homebrew/bin/claude".to_string(),
    ];
    for c in &candidates {
        if std::path::Path::new(c).exists() {
            return c.clone();
        }
    }
    "claude".to_string()
}

fn resolve_codex_bin(conn: &rusqlite::Connection) -> Option<String> {
    match std::env::var("SDR_CODEX_BIN").as_deref() {
        Ok("none") => return Some("none".into()),
        Ok(p) => return Some(p.to_string()),
        _ => {}
    }
    if let Ok(Some(saved)) = db::get_config(conn, "codex_bin") {
        if std::path::Path::new(&saved).exists() {
            return Some(saved);
        }
    }
    let path = agents::process::resolve("codex")?;
    let _ = db::set_config(conn, "codex_bin", &path);
    Some(path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args: Vec<String> = std::env::args().collect();
    let triggered = args.iter().any(|a| a == "--triggered");
    let debug_day = args.iter().any(|a| a == "--debug-day");
    let context = tauri::generate_context!();
    log::info!(
        "starting {} with {} configured window(s); debug study mode: {}",
        context.config().identifier,
        context.config().app.windows.len(),
        debug_day
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // A second launch (e.g. launchd firing while we run) just surfaces the window.
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
            let state = app.state::<AppState>();
            let classroom_slots = {
                let conn = state.db.0.lock().unwrap();
                let _ = classroom::refresh_appointments(&conn, &state.today());
                classroom::slot_views(&conn, &state.today(), state.debug_day)
            };
            if let Ok(slots) = classroom_slots {
                let due = slots
                    .into_iter()
                    .filter(|slot| slot.owed)
                    .collect::<Vec<_>>();
                if !due.is_empty() {
                    let _ = app.emit("classroom:owed", due);
                }
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            log::info!("initializing local study storage");
            let data_dir = app.path().app_data_dir().expect("app data dir resolvable");
            std::fs::create_dir_all(&data_dir)?;
            let conn = db::open(&data_dir.join("roulette.db"))?;
            db::seed_concepts(&conn, SEED_CONCEPTS)?;
            let startup_today = chrono::Local::now().format("%Y-%m-%d").to_string();
            language::initialize(&conn, &startup_today).map_err(std::io::Error::other)?;
            classroom::initialize(&conn).map_err(std::io::Error::other)?;
            // One-time, idempotent import of finished daily-routine history into
            // the shared runtime. Open work stays with the compatibility engine.
            match storage::primary_import::apply(&conn) {
                Ok(summary) => log::info!(
                    "legacy study import: {} imported, {} already imported, {} retained for recovery, {} open",
                    summary.imported.len(),
                    summary.already_imported,
                    summary.retained.len(),
                    summary.open_work
                ),
                Err(error) => log::error!("legacy study import skipped: {error}"),
            }
            let codex_bin = resolve_codex_bin(&conn);
            // Primary model for course generation: config 'model' (default opus).
            // Held behind Arc<Mutex> so settings changes apply live.
            let model = std::sync::Arc::new(Mutex::new(
                db::get_config(&conn, "model")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "opus".to_string()),
            ));
            let agent = std::sync::Arc::new(Mutex::new(
                db::get_config(&conn, "agent")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "claude".to_string()),
            ));
            let custom_bin = std::sync::Arc::new(Mutex::new(
                db::get_config(&conn, "custom_agent_bin")
                    .ok()
                    .flatten()
                    .unwrap_or_default(),
            ));
            // Live agent-activity feed: generator -> broadcast -> gen:log events.
            let (log_tx, mut log_rx) = tokio::sync::broadcast::channel::<String>(64);
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    while let Ok(line) = log_rx.recv().await {
                        let _ = handle.emit("gen:log", line);
                    }
                });
            }
            let mut generator = generator::Generator::new(
                resolve_claude_bin(),
                codex_bin,
                data_dir.join("scratch"),
                model,
                agent,
                custom_bin,
                Some(log_tx),
            );
            generator.runner.database = Some(data_dir.join("roulette.db"));
            app.manage(AppState {
                db: db::Db(Mutex::new(conn)),
                generator,
                data_dir,
                locked: AtomicBool::new(false),
                reading_remaining: AtomicI64::new(0),
                reading_owner: Mutex::new(None),
                timer_running: AtomicBool::new(false),
                timer_paused: AtomicBool::new(false),
                debug_day,
                escape_failures: Mutex::new(Vec::new()),
                prev_muted: Mutex::new(None),
                frontend_ready: AtomicBool::new(false),
                gen_notify: tokio::sync::Notify::new(),
                class_start_gate: tokio::sync::Mutex::new(()),
                chat_threads: Mutex::new(std::collections::HashMap::new()),
            });

            // Self-heal the launchd plist if it points at a stale binary path
            // (e.g. setup completed from a dev build).
            if !debug_day {
                let state = app.state::<AppState>();
                let conn = state.db.0.lock().unwrap();
                let onboarded =
                    matches!(db::get_config(&conn, "onboarded"), Ok(Some(v)) if v == "1");
                let paused =
                    matches!(db::get_config(&conn, "schedule_paused"), Ok(Some(v)) if v == "1");
                if onboarded {
                    let times = if paused {
                        Ok(Vec::new())
                    } else {
                        classroom::all_schedule_times(&conn)
                    };
                    drop(conn);
                    match times {
                        Ok(times) => scheduler::ensure_current_many(&times),
                        Err(error) => log::error!("could not reconcile class wakeups: {error}"),
                    }
                }
            }

            // Background generation worker.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                session::generation_worker(handle).await;
            });

            // Owed-session watcher: checks every 60s (covers app-already-running case).
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    let state = handle.state::<AppState>();
                    let classroom_slots = {
                        let conn = state.db.0.lock().unwrap();
                        let _ = classroom::refresh_appointments(&conn, &state.today());
                        classroom::slot_views(&conn, &state.today(), state.debug_day)
                    };
                    if let Ok(slots) = classroom_slots {
                        let due = slots
                            .into_iter()
                            .filter(|slot| slot.owed)
                            .collect::<Vec<_>>();
                        if !due.is_empty() {
                            // Classroom reminders are deliberately advisory:
                            // surface them in-app without taking the kiosk lock.
                            let _ = handle.emit("classroom:owed", due);
                        }
                    }
                }
            });

            // Launched by launchd at the scheduled time (or at load for catch-up).
            if triggered {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                    let state = handle.state::<AppState>();
                    let classroom_slots = {
                        let conn = state.db.0.lock().unwrap();
                        let _ = classroom::refresh_appointments(&conn, &state.today());
                        classroom::slot_views(&conn, &state.today(), state.debug_day)
                    };
                    if let Ok(slots) = classroom_slots {
                        let due = slots
                            .into_iter()
                            .filter(|slot| slot.owed)
                            .collect::<Vec<_>>();
                        if !due.is_empty() {
                            let _ = handle.emit("classroom:owed", due);
                        }
                    }
                    // Kick the pregen queue on every triggered launch (wake catch-up).
                    state.gen_notify.notify_one();
                });
            }
            log::info!("local study runtime ready");
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                let state = window.app_handle().state::<AppState>();
                if !state.debug_day && state.locked.load(Ordering::SeqCst) {
                    api.prevent_close();
                }
            }
            tauri::WindowEvent::Focused(false) => {
                let state = window.app_handle().state::<AppState>();
                if !state.debug_day && state.locked.load(Ordering::SeqCst) {
                    let _ = window.set_focus();
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::mark_frontend_ready,
            commands::get_app_state,
            commands::get_catalog,
            commands::get_enrollment_options,
            commands::get_enrollment_draft,
            commands::save_enrollment_draft,
            commands::placement::get_placement_check,
            commands::placement::start_placement_check,
            commands::placement::save_placement_response,
            commands::placement::submit_placement_round,
            commands::placement::continue_placement_check,
            commands::placement::finish_placement_check,
            commands::placement::get_path_recommendation,
            commands::check_agent,
            commands::agents::list_agent_runners,
            commands::agents::get_runner_models,
            commands::classes::get_class_path,
            commands::classes::accept_class_path,
            commands::agents::get_runner_configuration,
            commands::agents::save_runner_configuration,
            commands::agents::set_runner_key,
            commands::agents::get_local_models,
            commands::agents::get_local_pulls,
            commands::agents::install_local_runner,
            commands::agents::start_local_runner,
            commands::agents::pull_local_model,
            commands::agents::remove_local_model,
            commands::agents::select_runner,
            commands::agents::set_openrouter_free_only,
            commands::agents::get_openrouter_free_only,
            commands::agents::test_agent_connection,
            commands::agents::get_agent_activity,
            commands::agents::get_agent_policy,
            commands::agents::set_agent_policy,
            commands::complete_setup,
            commands::get_curriculum_map,
            commands::configure_classroom_program,
            commands::upsert_classroom_slot,
            commands::delete_classroom_slot,
            commands::plan_classroom_schedule,
            commands::start_classroom_session,
            commands::resume_classroom_session,
            commands::submit_classroom_engineering_session,
            commands::save_class_lesson_work,
            commands::save_class_check_answer,
            commands::submit_class_check,
            commands::submit_class_language_check,
            commands::pause_class_lesson,
            commands::skip_class_lesson,
            commands::skip_appointment,
            commands::get_class_appointments,
            commands::submit_language_session,
            commands::set_kiosk_level,
            commands::set_model,
            commands::set_agent,
            commands::set_deepseek_api_key,
            commands::pause_schedule,
            commands::resume_schedule,
            commands::get_quiz,
            commands::submit_answer,
            commands::finish_quiz,
            commands::get_review,
            commands::finish_review,
            commands::get_roulette,
            commands::complete_track_day,
            commands::ensure_course,
            commands::start_course,
            commands::finish_course,
            commands::ensure_audio,
            commands::get_audio_enabled,
            commands::set_audio_enabled,
            commands::get_exit_quiz,
            commands::submit_exit_quiz,
            commands::escape_session,
            commands::get_escape_phrase,
            commands::get_dashboard,
            commands::get_progress_lesson,
            commands::get_past_course,
            commands::open_resources,
            commands::get_exercise,
            commands::save_exercise_draft,
            commands::save_exercise_completion,
            commands::get_chat,
            commands::send_chat_message,
        ])
        .build(context)
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Ready = event {
                for (label, window) in app.webview_windows() {
                    log::info!(
                        "desktop window {label}: visible={:?}, size={:?}",
                        window.is_visible(),
                        window.inner_size()
                    );
                }
            }
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                let state = app.state::<AppState>();
                if !state.debug_day && state.locked.load(Ordering::SeqCst) {
                    api.prevent_exit();
                }
            }
        });
}

//! Explicit native-backend QA: run the real engineering lesson pipeline in a
//! disposable database. No webview, scheduler, focus enforcement or seed edits.
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicI64},
        Arc, Mutex,
    },
};
use system_design_roulette_lib::{classroom, db, generator::Generator, state::AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 || args[2] != "--live" {
        return Err("usage: lesson_probe DISPOSABLE_QA_DATABASE SUBJECT_ID --live".into());
    }
    let path = PathBuf::from(&args[0]).canonicalize()?;
    let data_dir = path.parent().ok_or("database has no parent")?.to_owned();
    let conn = rusqlite::Connection::open(&path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.pragma_update(None, "foreign_keys", true)?;
    let program = classroom::program_row(&conn, &args[1])?;
    if classroom::active_engineering_session(&conn, &args[1])?.is_some() {
        return Err("This QA class already has an active lesson. Use a fresh disposable QA database so a resumed lesson cannot be mistaken for a new generation.".into());
    }
    if !classroom::has_enabled_schedule(&conn, &args[1])? {
        return Err("QA class needs an enabled study time".into());
    }
    let (log_tx, mut log_rx) = tokio::sync::broadcast::channel::<String>(100);
    let logger = tokio::spawn(async move {
        while let Ok(line) = log_rx.recv().await {
            println!("{line}");
        }
    });
    let mut generator = Generator::new(
        "claude".into(),
        Some("codex".into()),
        data_dir.join("scratch"),
        Arc::new(Mutex::new(program.model.clone())),
        Arc::new(Mutex::new(program.agent.clone())),
        Arc::new(Mutex::new(program.custom_agent_bin.clone())),
        Some(log_tx),
    );
    generator.runner.database = Some(path);
    let state = AppState {
        db: db::Db(Mutex::new(conn)),
        generator,
        data_dir,
        locked: AtomicBool::new(false),
        reading_remaining: AtomicI64::new(0),
        reading_owner: Mutex::new(None),
        timer_running: AtomicBool::new(false),
        timer_paused: AtomicBool::new(false),
        debug_day: true,
        escape_failures: Mutex::new(vec![]),
        prev_muted: Mutex::new(None),
        frontend_ready: AtomicBool::new(false),
        gen_notify: tokio::sync::Notify::new(),
        class_start_gate: tokio::sync::Mutex::new(()),
        chat_threads: Mutex::new(Default::default()),
        focus: Default::default(),
    };
    println!(
        "{}",
        serde_json::json!({"subject":args[1],"runner":program.agent,"model":program.model})
    );
    let result = classroom::start_engineering_session(&state, &args[1], None, false).await;
    logger.abort();
    match result {
        Ok(lesson) => println!(
            "{}",
            serde_json::json!({"saved_session_id":lesson.session_id,"subject":lesson.subject_id,"title":lesson.title})
        ),
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

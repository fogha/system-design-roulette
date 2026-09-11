//! Commands for taking a profile off this machine and putting one back.

use crate::domain::portability::{self, ArchiveSummary};
use crate::state::AppState;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

type CmdResult<T> = Result<T, String>;
fn err<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

#[derive(Debug, Serialize)]
pub struct ExportResult {
    pub path: String,
    pub summary: ArchiveSummary,
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub rows: usize,
    /// Where the record that was replaced is kept, so it can be restored.
    pub backup_path: String,
    pub summary: ArchiveSummary,
}

/// Where an export goes: the learner's documents if that exists, otherwise
/// beside the database, which always does.
fn export_directory(app: &AppHandle, state: &AppState) -> std::path::PathBuf {
    let documents = app
        .path()
        .document_dir()
        .ok()
        .map(|dir| dir.join("Principia Desk"));
    match documents {
        Some(dir) if std::fs::create_dir_all(&dir).is_ok() => dir,
        _ => state.data_dir.join("exports"),
    }
}

#[tauri::command]
pub fn export_profile(app: AppHandle, state: State<'_, AppState>) -> CmdResult<ExportResult> {
    let archive = {
        let conn = state.db.0.lock().unwrap();
        portability::export(&conn, env!("CARGO_PKG_VERSION")).map_err(err)?
    };
    let summary = portability::summarize(&archive);
    let directory = export_directory(&app, &state);
    std::fs::create_dir_all(&directory).map_err(err)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = directory.join(format!("principia-desk-{stamp}.json"));
    let document = serde_json::to_string(&archive).map_err(err)?;
    std::fs::write(&path, document).map_err(err)?;
    Ok(ExportResult {
        path: path.to_string_lossy().into_owned(),
        summary,
    })
}

/// Read an archive and report what it holds. Nothing is written, so a learner
/// can see what a file would replace before agreeing to it.
#[tauri::command]
pub fn inspect_archive(document: String) -> CmdResult<ArchiveSummary> {
    portability::read(&document)
        .map(|archive| portability::summarize(&archive))
        .map_err(err)
}

#[tauri::command]
pub fn import_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    document: String,
) -> CmdResult<ImportResult> {
    // A focused session owns the desk and its lesson rows; replacing the
    // record underneath it would strand both.
    if crate::enforcement::view(&state).is_some() {
        return Err("Finish, pause or skip the active session before importing a profile.".into());
    }
    let archive = portability::read(&document).map_err(err)?;
    let summary = portability::summarize(&archive);
    let database = state.data_dir.join("principia.db");
    let backup = crate::storage::migrations::publish_backup(&database, "before-import")
        .map_err(|error| format!("the profile was not touched: {error}"))?;
    let rows = {
        let mut conn = state.db.0.lock().unwrap();
        portability::restore(&mut conn, &archive).map_err(err)?
    };
    state.clear_chat_threads();
    let _ = app.emit("classroom:state", serde_json::json!({ "refresh": true }));
    Ok(ImportResult {
        rows,
        backup_path: backup.to_string_lossy().into_owned(),
        summary,
    })
}

/// Reveal an exported file in the system file browser.
#[tauri::command]
pub fn reveal_export(app: AppHandle, path: String) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().reveal_item_in_dir(&path).map_err(err)
}

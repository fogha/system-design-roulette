//! The class builder's commands: a learner's own class from brief to
//! published course, the tutor's drafting and review, the source check,
//! and the class file.

use super::{err, CmdResult};
use crate::{
    class_builder,
    domain::custom::{
        self, ClassFile, CourseBrief, CourseDraft, CustomCourseSummary, CustomCourseView,
    },
    state::AppState,
};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, State};

fn tell(app: &AppHandle) {
    let _ = app.emit("classroom:state", serde_json::json!({ "refresh": true }));
}

#[tauri::command]
pub fn list_custom_courses(state: State<'_, AppState>) -> CmdResult<Vec<CustomCourseSummary>> {
    custom::list(&state.db.0.lock().unwrap()).map_err(err)
}

#[tauri::command]
pub fn get_custom_course(state: State<'_, AppState>, id: String) -> CmdResult<CustomCourseView> {
    custom::get(&state.db.0.lock().unwrap(), &id).map_err(err)
}

/// Start a class from a brief. `origin` says how the draft will be made:
/// by the tutor, by hand, or from a file.
#[tauri::command]
pub fn create_custom_course(
    state: State<'_, AppState>,
    brief: CourseBrief,
    origin: String,
) -> CmdResult<CustomCourseView> {
    custom::create(&state.db.0.lock().unwrap(), &brief, &origin).map_err(err)
}

#[tauri::command]
pub fn save_custom_course_brief(
    state: State<'_, AppState>,
    id: String,
    brief: CourseBrief,
) -> CmdResult<CustomCourseView> {
    custom::save_brief(&state.db.0.lock().unwrap(), &id, &brief).map_err(err)
}

#[tauri::command]
pub fn save_custom_course_draft(
    state: State<'_, AppState>,
    id: String,
    draft: CourseDraft,
) -> CmdResult<CustomCourseView> {
    custom::save_draft(&state.db.0.lock().unwrap(), &id, draft).map_err(err)
}

/// Ask the class's tutor to draft the curriculum. The result replaces the
/// draft; the issues that remain come back with the view.
#[tauri::command]
pub async fn draft_custom_course(
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<CustomCourseView> {
    let (brief, _) = {
        let conn = state.db.0.lock().unwrap();
        let view = custom::get(&conn, &id).map_err(err)?;
        (view.brief, view.draft)
    };
    let _run = state.generator.feed.begin(&format!("draft:{id}"), &id);
    let (draft, _issues, _source) = state
        .generator
        .draft_custom_course(&id, &brief)
        .await
        .map_err(|error| format!("the tutor could not draft the course: {error}"))?;
    let conn = state.db.0.lock().unwrap();
    custom::save_draft(&conn, &id, draft).map_err(err)
}

/// Ask the tutor to read the draft back. Findings are kept with the class.
#[tauri::command]
pub async fn review_custom_course(
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<CustomCourseView> {
    let (brief, draft) = {
        let conn = state.db.0.lock().unwrap();
        let view = custom::get(&conn, &id).map_err(err)?;
        (view.brief, view.draft)
    };
    let _run = state.generator.feed.begin(&format!("review:{id}"), &id);
    let (findings, _) = state
        .generator
        .review_custom_course(&brief, &draft)
        .await
        .map_err(|error| format!("the tutor could not review the course: {error}"))?;
    let conn = state.db.0.lock().unwrap();
    custom::save_review(&conn, &id, &findings).map_err(err)
}

/// Fetch every primary source and keep the result with the class.
#[tauri::command]
pub async fn verify_custom_course_sources(
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<CustomCourseView> {
    let draft = {
        let conn = state.db.0.lock().unwrap();
        custom::get(&conn, &id).map_err(err)?.draft
    };
    let _run = state.generator.feed.begin(&format!("sources:{id}"), &id);
    let checks =
        class_builder::verify_sources(&state.generator.researcher, &state.generator.feed, &draft)
            .await;
    let conn = state.db.0.lock().unwrap();
    custom::save_sources(&conn, &id, &checks).map_err(err)
}

/// Publish the draft as a course. The desk refreshes its catalog view.
#[tauri::command]
pub fn publish_custom_course(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<CustomCourseView> {
    let view = custom::publish(&state.db.0.lock().unwrap(), &id).map_err(err)?;
    state.generator.feed.say(format!(
        "class builder: published {} v{}",
        view.draft.label, view.version
    ));
    tell(&app);
    Ok(view)
}

#[tauri::command]
pub fn delete_custom_course_draft(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<()> {
    custom::delete_draft(&state.db.0.lock().unwrap(), &id).map_err(err)?;
    tell(&app);
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct ClassExport {
    pub path: String,
    pub file_name: String,
}

/// Classes are filed under `Documents/Principia Desk/classes/`.
fn classes_directory(app: &AppHandle, state: &AppState) -> PathBuf {
    let documents = app
        .path()
        .document_dir()
        .ok()
        .map(|dir| dir.join("Principia Desk").join("classes"));
    match documents {
        Some(dir) if std::fs::create_dir_all(&dir).is_ok() => dir,
        _ => {
            let dir = state.data_dir.join("exports").join("classes");
            let _ = std::fs::create_dir_all(&dir);
            dir
        }
    }
}

/// Write the class file: the brief and the draft, nothing personal.
#[tauri::command]
pub fn export_custom_course(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<ClassExport> {
    let file = custom::export(&state.db.0.lock().unwrap(), &id).map_err(err)?;
    let stem: String = file
        .draft
        .label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim()
        .to_string();
    let stem = if stem.is_empty() { id.clone() } else { stem };
    let directory = classes_directory(&app, &state);
    let mut path = directory.join(format!("{stem}.principia-class.json"));
    if path.exists() {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        path = directory.join(format!("{stem}-{stamp}.principia-class.json"));
    }
    let body = serde_json::to_string_pretty(&file).map_err(err)?;
    std::fs::write(&path, body).map_err(err)?;
    Ok(ClassExport {
        file_name: path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default(),
        path: path.to_string_lossy().to_string(),
    })
}

/// Read a class file, given as its text (the frontend reads the file the
/// learner picked) and make a new draft from it.
#[tauri::command]
pub fn import_custom_course(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> CmdResult<CustomCourseView> {
    if text.len() > 4_000_000 {
        return Err("that file is too large to be a class".into());
    }
    let file: ClassFile =
        serde_json::from_str(&text).map_err(|error| format!("not a class file: {error}"))?;
    let view = custom::import(&state.db.0.lock().unwrap(), file).map_err(err)?;
    tell(&app);
    Ok(view)
}

//! The class builder's commands: a learner's own class from brief to
//! published course, the tutor's drafting and review, the source check,
//! and the class file.

use super::{err, CmdResult};
use crate::{
    class_builder::{self, BuilderJobs},
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
pub fn list_custom_courses(
    state: State<'_, AppState>,
    jobs: State<'_, BuilderJobs>,
) -> CmdResult<Vec<CustomCourseSummary>> {
    let mut list = custom::list(&state.db.0.lock().unwrap()).map_err(err)?;
    for item in &mut list {
        item.working = jobs.working(&item.id);
    }
    Ok(list)
}

#[tauri::command]
pub fn get_custom_course(
    state: State<'_, AppState>,
    jobs: State<'_, BuilderJobs>,
    id: String,
) -> CmdResult<CustomCourseView> {
    let mut view = custom::get(&state.db.0.lock().unwrap(), &id).map_err(err)?;
    view.working = jobs.working(&id);
    view.working_at = jobs.working_at(&id);
    Ok(view)
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
    app: AppHandle,
    state: State<'_, AppState>,
    jobs: State<'_, BuilderJobs>,
    id: String,
) -> CmdResult<CustomCourseView> {
    let (brief, _) = {
        let conn = state.db.0.lock().unwrap();
        let view = custom::get(&conn, &id).map_err(err)?;
        (view.brief, view.draft)
    };
    if jobs.working(&id).is_some() {
        return Err("the tutor is already working on this class".into());
    }
    let job = jobs.begin(&id, "draft");
    let _run = state.generator.feed.begin(&format!("draft:{id}"), &id);
    let drafted = state
        .generator
        .draft_custom_course(&id, &brief)
        .await
        .map_err(|error| format!("the tutor could not draft the course: {error}"))
        .inspect_err(|error| {
            state
                .generator
                .feed
                .say(format!("class builder: the draft failed: {error}"))
        });
    // Save while the job is still marked, then tell the desk: a refresh that
    // arrives between the two would show the old draft as finished.
    let saved = drafted.and_then(|(draft, _issues, _source)| {
        let conn = state.db.0.lock().unwrap();
        custom::save_draft(&conn, &id, draft).map_err(err)
    });
    drop(job);
    tell(&app);
    saved
}

/// Ask the tutor to read the draft back. Findings are kept with the class.
#[tauri::command]
pub async fn review_custom_course(
    app: AppHandle,
    state: State<'_, AppState>,
    jobs: State<'_, BuilderJobs>,
    id: String,
) -> CmdResult<CustomCourseView> {
    let (brief, draft, settled) = {
        let conn = state.db.0.lock().unwrap();
        let view = custom::get(&conn, &id).map_err(err)?;
        let settled: Vec<_> = view
            .review
            .into_iter()
            .filter(|f| f.status != "open")
            .collect();
        (view.brief, view.draft, settled)
    };
    if jobs.working(&id).is_some() {
        return Err("the tutor is already working on this class".into());
    }
    let job = jobs.begin(&id, "review");
    let _run = state.generator.feed.begin(&format!("review:{id}"), &id);
    let reviewed = state
        .generator
        .review_custom_course(&brief, &draft, &settled)
        .await
        .map_err(|error| format!("the tutor could not review the course: {error}"))
        .inspect_err(|error| {
            state
                .generator
                .feed
                .say(format!("class builder: the review failed: {error}"))
        });
    let saved = reviewed.and_then(|(findings, _)| {
        let conn = state.db.0.lock().unwrap();
        custom::save_review(&conn, &id, &findings).map_err(err)
    });
    drop(job);
    tell(&app);
    saved
}

/// Ask the tutor to make the change one finding asks for. The draft is
/// replaced by the patched one and the finding is marked fixed with the
/// tutor's note; the other findings stay as they were.
#[tauri::command]
pub async fn fix_custom_course_finding(
    app: AppHandle,
    state: State<'_, AppState>,
    jobs: State<'_, BuilderJobs>,
    id: String,
    index: usize,
) -> CmdResult<CustomCourseView> {
    let (brief, draft, finding) = {
        let conn = state.db.0.lock().unwrap();
        let view = custom::get(&conn, &id).map_err(err)?;
        let finding = view
            .review
            .get(index)
            .cloned()
            .ok_or_else(|| "that finding is not in the review".to_string())?;
        (view.brief, view.draft, finding)
    };
    if jobs.working(&id).is_some() {
        return Err("the tutor is already working on this class".into());
    }
    let job = jobs.begin(&id, "fix");
    jobs.focus(&id, Some(index));
    let _run = state.generator.feed.begin(&format!("fix:{id}"), &id);
    let fixed = state
        .generator
        .fix_custom_course_finding(&brief, &draft, &finding)
        .await
        .map_err(|error| format!("the tutor could not make the change: {error}"))
        .inspect_err(|error| {
            state
                .generator
                .feed
                .say(format!("class builder: the change failed: {error}"))
        });
    let saved = fixed.and_then(|(changed, note, _)| {
        let conn = state.db.0.lock().unwrap();
        custom::save_draft(&conn, &id, changed).map_err(err)?;
        let note = if note.is_empty() {
            "by the tutor".to_string()
        } else {
            format!("by the tutor: {note}")
        };
        custom::resolve_finding(&conn, &id, index, "fixed", &note).map_err(err)
    });
    drop(job);
    tell(&app);
    saved
}

/// Ask the tutor to settle every open finding, one after another, each
/// change applied and saved before the next so later fixes see earlier
/// ones. A finding whose change fails stays open; the rest go on.
#[tauri::command]
pub async fn fix_all_custom_course_findings(
    app: AppHandle,
    state: State<'_, AppState>,
    jobs: State<'_, BuilderJobs>,
    id: String,
) -> CmdResult<CustomCourseView> {
    let open: Vec<usize> = {
        let conn = state.db.0.lock().unwrap();
        let view = custom::get(&conn, &id).map_err(err)?;
        view.review
            .iter()
            .enumerate()
            .filter(|(_, f)| f.status == "open")
            .map(|(i, _)| i)
            .collect()
    };
    if open.is_empty() {
        return Err("every finding is already settled".into());
    }
    if jobs.working(&id).is_some() {
        return Err("the tutor is already working on this class".into());
    }
    let job = jobs.begin(&id, "fix");
    let _run = state.generator.feed.begin(&format!("fix:{id}"), &id);
    let total = open.len();
    let mut failed = 0usize;
    for (n, index) in open.into_iter().enumerate() {
        let (brief, draft, finding) = {
            let conn = state.db.0.lock().unwrap();
            let view = custom::get(&conn, &id).map_err(err)?;
            let Some(finding) = view.review.get(index).cloned() else {
                continue;
            };
            (view.brief, view.draft, finding)
        };
        jobs.focus(&id, Some(index));
        tell(&app);
        state.generator.feed.say(format!(
            "class builder: settling finding {} of {total}",
            n + 1
        ));
        match state
            .generator
            .fix_custom_course_finding(&brief, &draft, &finding)
            .await
        {
            Ok((changed, note, _)) => {
                let conn = state.db.0.lock().unwrap();
                custom::save_draft(&conn, &id, changed).map_err(err)?;
                let note = if note.is_empty() {
                    "by the tutor".to_string()
                } else {
                    format!("by the tutor: {note}")
                };
                custom::resolve_finding(&conn, &id, index, "fixed", &note).map_err(err)?;
            }
            Err(error) => {
                failed += 1;
                state.generator.feed.say(format!(
                    "class builder: finding {} of {total} stays open; the change failed: {error}",
                    n + 1
                ));
            }
        }
        tell(&app);
    }
    state.generator.feed.say(format!(
        "class builder: {} of {total} finding(s) settled by the tutor",
        total - failed
    ));
    let view = custom::get(&state.db.0.lock().unwrap(), &id).map_err(err);
    drop(job);
    tell(&app);
    view
}

/// Settle a finding by hand: `fixed` after editing, `dismissed` with a
/// reason, or `open` again.
#[tauri::command]
pub fn resolve_custom_course_finding(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    index: usize,
    status: String,
    note: String,
) -> CmdResult<CustomCourseView> {
    let view = custom::resolve_finding(&state.db.0.lock().unwrap(), &id, index, &status, &note)
        .map_err(err)?;
    tell(&app);
    Ok(view)
}

/// Keep an unreachable source knowingly, or withdraw that.
#[tauri::command]
pub fn accept_custom_course_source(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    url: String,
    accepted: bool,
) -> CmdResult<CustomCourseView> {
    let view =
        custom::accept_source(&state.db.0.lock().unwrap(), &id, &url, accepted).map_err(err)?;
    tell(&app);
    Ok(view)
}

/// The learner confirms their own read-through of the draft as it stands.
#[tauri::command]
pub fn mark_custom_course_read(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    read: bool,
) -> CmdResult<CustomCourseView> {
    let view = custom::mark_read(&state.db.0.lock().unwrap(), &id, read).map_err(err)?;
    tell(&app);
    Ok(view)
}

/// Fetch every primary source and keep the result with the class.
#[tauri::command]
pub async fn verify_custom_course_sources(
    app: AppHandle,
    state: State<'_, AppState>,
    jobs: State<'_, BuilderJobs>,
    id: String,
) -> CmdResult<CustomCourseView> {
    let draft = {
        let conn = state.db.0.lock().unwrap();
        custom::get(&conn, &id).map_err(err)?.draft
    };
    if jobs.working(&id).is_some() {
        return Err("the tutor is already working on this class".into());
    }
    let job = jobs.begin(&id, "sources");
    let _run = state.generator.feed.begin(&format!("sources:{id}"), &id);
    let checks =
        class_builder::verify_sources(&state.generator.researcher, &state.generator.feed, &draft)
            .await;
    let saved = {
        let conn = state.db.0.lock().unwrap();
        custom::save_sources(&conn, &id, &checks).map_err(err)
    };
    drop(job);
    tell(&app);
    saved
}

/// Ask the tutor to write the question bank: three cited questions per
/// stage. It is kept with the class and registered when the class is
/// published, which turns on the placement check and unit challenges.
#[tauri::command]
pub async fn write_custom_course_bank(
    app: AppHandle,
    state: State<'_, AppState>,
    jobs: State<'_, BuilderJobs>,
    id: String,
) -> CmdResult<CustomCourseView> {
    let (brief, draft) = {
        let conn = state.db.0.lock().unwrap();
        let view = custom::get(&conn, &id).map_err(err)?;
        if !view.issues.is_empty() {
            return Err(format!(
                "fix the draft before writing its questions: {}",
                view.issues[0].message
            ));
        }
        (view.brief, view.draft)
    };
    if jobs.working(&id).is_some() {
        return Err("the tutor is already working on this class".into());
    }
    let job = jobs.begin(&id, "bank");
    let _run = state.generator.feed.begin(&format!("bank:{id}"), &id);
    let written = state
        .generator
        .write_custom_course_bank(&brief, &draft)
        .await
        .map_err(|error| format!("the tutor could not write the question bank: {error}"))
        .inspect_err(|error| {
            state
                .generator
                .feed
                .say(format!("class builder: the question bank failed: {error}"))
        });
    let saved = written.and_then(|(bank, _)| {
        let conn = state.db.0.lock().unwrap();
        custom::save_bank(&conn, &id, &bank).map_err(err)
    });
    drop(job);
    tell(&app);
    saved
}

/// A learner disputes a written key. The question stops counting and is
/// left out of every sample until it is corrected.
#[tauri::command]
pub fn void_custom_question(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    question_id: String,
    reason: String,
) -> CmdResult<CustomCourseView> {
    let view = custom::void_question(&state.db.0.lock().unwrap(), &id, &question_id, &reason)
        .map_err(err)?;
    state.generator.feed.say(format!(
        "class builder: a key in {} was disputed ({question_id}); the question is set aside",
        view.draft.label
    ));
    tell(&app);
    Ok(view)
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

//! The menu bar desk.
//!
//! The app stays resident: closing or hiding the window leaves this icon and
//! its menu, which say what is due, what is next, what today holds, and offer
//! the only two things an alarm accepts: starting the lesson, or a snooze.
//! Quit lives here too, and is withheld while an alarm rings or a focused
//! session holds the desk, so the background runner cannot be dismissed as a
//! way around either.

use crate::alarm::{self, AlarmView};
use crate::state::AppState;
use std::sync::atomic::Ordering;
use tauri::{
    menu::{Menu, MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};

pub const TRAY_ID: &str = "desk";

fn label_item(
    app: &AppHandle,
    id: &str,
    text: &str,
) -> tauri::Result<tauri::menu::MenuItem<tauri::Wry>> {
    MenuItemBuilder::with_id(id, text).enabled(false).build(app)
}

fn clock(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|t| t.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_else(|_| value.to_string())
}

/// Build the menu for the current state. Rebuilt whole on every refresh, which
/// is simpler than editing items and never leaves a stale line behind.
fn build(app: &AppHandle, alarm: Option<&AlarmView>) -> tauri::Result<Menu<tauri::Wry>> {
    let state = app.state::<AppState>();
    let (paused, slots, appointments) = {
        let conn = state.db.0.lock().unwrap();
        let today = state.today();
        (
            matches!(crate::db::get_config(&conn, "schedule_paused"), Ok(Some(v)) if v == "1"),
            crate::classroom::slot_views(&conn, &today, state.debug_day).unwrap_or_default(),
            crate::classroom::refresh_appointments(&conn, &today).unwrap_or_default(),
        )
    };
    let mut menu = MenuBuilder::new(app);

    if let Some(alarm) = alarm {
        let headline = match &alarm.snoozed_until {
            Some(until) => format!("{} · due, snoozed until {}", alarm.label, clock(until)),
            None => format!("{} · due now", alarm.label),
        };
        menu = menu.item(&label_item(app, "alarm", &headline)?);
        if alarm.queued > 0 {
            menu = menu.item(&label_item(
                app,
                "queued",
                &format!("{} more waiting behind it", alarm.queued),
            )?);
        }
        menu = menu.item(
            &MenuItemBuilder::with_id(
                format!("start:{}", alarm.occurrence_id),
                format!("Start {}", alarm.label),
            )
            .build(app)?,
        );
        if alarm.snoozed_until.is_none() {
            for minutes in alarm::SNOOZE_MINUTES {
                menu = menu.item(
                    &MenuItemBuilder::with_id(
                        format!("snooze:{}:{minutes}", alarm.occurrence_id),
                        format!("Snooze {minutes} min"),
                    )
                    .build(app)?,
                );
            }
        }
        menu = menu.separator();
    } else if paused {
        menu = menu.item(&label_item(app, "next", "Appointments paused")?);
        menu = menu.separator();
    } else {
        let next = slots
            .iter()
            .filter(|slot| slot.enabled && !slot.owed && !slot.next_fire_at.is_empty())
            .min_by(|a, b| a.next_fire_at.cmp(&b.next_fire_at));
        let text = match next {
            Some(slot) => {
                let when = chrono::DateTime::parse_from_rfc3339(&slot.next_fire_at)
                    .map(|t| {
                        let local = t.with_timezone(&chrono::Local);
                        if local.date_naive() == chrono::Local::now().date_naive() {
                            format!("today {}", local.format("%H:%M"))
                        } else {
                            local.format("%a %H:%M").to_string()
                        }
                    })
                    .unwrap_or_else(|_| slot.next_fire_at.clone());
                format!("Next: {} · {when}", slot.label)
            }
            None => "No study times set".to_string(),
        };
        menu = menu.item(&label_item(app, "next", &text)?);
        menu = menu.separator();
    }

    if !appointments.is_empty() {
        menu = menu.item(&label_item(app, "today", "Today")?);
        for appointment in appointments.iter().take(6) {
            let line = format!(
                "  {} {} · {}",
                appointment.local_time, appointment.label, appointment.disposition
            );
            menu = menu.item(&label_item(
                app,
                &format!("appt:{}", appointment.id),
                &line,
            )?);
        }
        menu = menu.separator();
    }

    menu = menu.item(&MenuItemBuilder::with_id("open", "Open Principia Desk").build(app)?);
    menu = if paused {
        menu.item(&MenuItemBuilder::with_id("resume", "Resume appointments").build(app)?)
    } else {
        menu.item(&MenuItemBuilder::with_id("pause", "Pause appointments").build(app)?)
    };
    menu = menu.separator();
    let ringing = alarm.is_some_and(|a| a.snoozed_until.is_none());
    let locked = state.locked.load(Ordering::SeqCst);
    menu = menu.item(
        &MenuItemBuilder::with_id("quit", "Quit Principia Desk")
            .enabled(!ringing && !locked)
            .build(app)?,
    );
    menu.build()
}

/// Bring the desk window back from the menu bar.
pub fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn handle(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();
    match id {
        "open" => show_window(app),
        "pause" => {
            if let Err(error) = crate::commands::pause_schedule(app.clone(), app.state()) {
                log::warn!("pause from the menu bar failed: {error}");
            }
            alarm::evaluate(app);
        }
        "resume" => {
            if let Err(error) = crate::commands::resume_schedule(app.clone(), app.state()) {
                log::warn!("resume from the menu bar failed: {error}");
            }
            alarm::evaluate(app);
        }
        "quit" => {
            if state.alarm_ringing.load(Ordering::SeqCst) || state.locked.load(Ordering::SeqCst) {
                log::warn!("quit refused: an alarm or a focused session holds the desk");
                return;
            }
            app.exit(0);
        }
        other => {
            if let Some(occurrence) = other.strip_prefix("start:") {
                if let Some(alarm) = alarm::current(&state) {
                    if alarm.occurrence_id == occurrence || alarm.queued > 0 {
                        show_window(app);
                        let _ = app.emit(
                            "tray:start",
                            serde_json::json!({
                                "course_id": alarm.course_id,
                                "occurrence_id": occurrence,
                            }),
                        );
                    }
                }
            } else if let Some(rest) = other.strip_prefix("snooze:") {
                if let Some((occurrence, minutes)) = rest.rsplit_once(':') {
                    match minutes
                        .parse::<u32>()
                        .map_err(|_| "bad snooze".to_string())
                        .and_then(|m| alarm::snooze(&state, occurrence, m))
                    {
                        Ok(_) => alarm::evaluate(app),
                        Err(error) => log::warn!("snooze from the menu bar failed: {error}"),
                    }
                }
            }
        }
    }
}

/// Install the icon once at startup. Later state changes go through `refresh`.
pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let menu = build(app, None)?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Principia Desk")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| handle(app, event.id().as_ref()));
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}

/// Rebuild the menu and the menu bar title from the current state.
pub fn refresh(app: &AppHandle, alarm: Option<&AlarmView>) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    match build(app, alarm) {
        Ok(menu) => {
            let _ = tray.set_menu(Some(menu));
        }
        Err(error) => log::warn!("menu bar refresh failed: {error}"),
    }
    let title = alarm.map(|a| {
        if a.snoozed_until.is_some() {
            format!("{} snoozed", a.label)
        } else {
            format!("{} due", a.label)
        }
    });
    let _ = tray.set_title(title.as_deref());
    let _ = tray.set_tooltip(Some(
        title
            .clone()
            .map(|t| format!("Principia Desk · {t}"))
            .unwrap_or_else(|| "Principia Desk".into()),
    ));
}

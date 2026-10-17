//! Running in the background: a tray icon, and (optionally) starting with Windows.
//!
//! Closing the window hides it instead of quitting, so the launcher keeps running, can start a game from the tray menu and
//! restores the game folder when the game closes. "Quit" in the tray menu really quits (it asks the user to close the
//! game first when one is running, because the game folder is changed while it runs).

use crate::{AppState, error::{Error, Result}};
use std::sync::atomic::Ordering;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

const TRAY: &str = "main";
/// The argument a start with Windows carries, so the window stays hidden then.
pub const MINIMIZED: &str = "--minimized";

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn menu(app: &AppHandle, open: &str, play: &str, quit: &str) -> tauri::Result<Menu<tauri::Wry>> {
    let open = MenuItem::with_id(app, "open", open, true, None::<&str>)?;
    let play = MenuItem::with_id(app, "play", play, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", quit, true, None::<&str>)?;
    Menu::with_items(app, &[&open, &play, &quit])
}

/// Quit for real, unless a game is running (then bring the window forward and say why).
fn quit(app: &AppHandle) {
    if app.state::<AppState>().launching.load(Ordering::SeqCst) {
        show_main(app);
        let _ = app.emit("close-blocked", ());
    } else {
        app.exit(0);
    }
}

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let mut tray = TrayIconBuilder::with_id(TRAY)
        .tooltip("GenLauncher")
        .menu(&menu(app, "Open GenLauncher", "Play", "Quit")?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "play" => {
                // the page starts the game with what is chosen there; it keeps running while hidden
                let _ = app.emit("tray-play", ());
            }
            "quit" => quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// The page tells the core whether closing the window should hide it, and the menu's words in the chosen language.
#[tauri::command]
pub(crate) fn background_set(app: AppHandle, state: State<'_, AppState>, close_to_tray: bool, open: String, play: String, quit: String) -> Result<()> {
    state.close_to_tray.store(close_to_tray, Ordering::SeqCst);
    if let Some(tray) = app.tray_by_id(TRAY) {
        let m = menu(&app, &open, &play, &quit).map_err(|e| Error::msg(e.to_string()))?;
        tray.set_menu(Some(m)).map_err(|e| Error::msg(e.to_string()))?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn autostart_status(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub(crate) fn autostart_set(app: AppHandle, enabled: bool) -> Result<()> {
    let manager = app.autolaunch();
    let result = if enabled { manager.enable() } else { manager.disable() };
    result.map_err(|e| Error::msg(format!("Could not change the start with Windows setting: {e}")))
}

/// A Windows notification (for things that happen while the window is hidden, such as updates found in the background).
#[tauri::command]
pub(crate) fn os_notify(app: AppHandle, title: String, body: String) -> Result<()> {
    use tauri_plugin_notification::NotificationExt;
    app.notification().builder().title(title).body(body).show().map_err(|e| Error::msg(format!("Could not show a notification: {e}")))
}

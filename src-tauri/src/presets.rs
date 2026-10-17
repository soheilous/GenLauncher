//! Presets are named combinations of a mod, its patches, add-ons and executables (kept by the interface). This is the part that
//! needs the operating system: a command line that starts a preset (`--play <id>`), and a desktop shortcut for it.

use crate::error::{Error, Result};
use crate::AppState;
use tauri::{AppHandle, Emitter, Manager, State};

/// The id after `--play` in a command line, if it is a plain id (letters, digits, `-`, `_`).
pub fn play_arg(args: &[String]) -> Option<String> {
    let i = args.iter().position(|a| a == "--play")?;
    let id = args.get(i + 1)?;
    (!id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')).then(|| id.clone())
}

/// Called when the program is started again with `--play <id>` while it is running.
pub fn play_in_running(app: &AppHandle, args: &[String]) {
    if let Some(id) = play_arg(args) {
        let _ = app.emit("play-preset", id);
    }
}

/// What the interface asks for once it is ready: the preset the program was started to play, once.
#[tauri::command]
pub(crate) fn take_pending_play(state: State<'_, AppState>) -> Option<String> {
    state.pending_play.lock().unwrap().take()
}

/// The command line that starts a preset (to paste into Steam's "Add a non-Steam game" launch options, or a shortcut).
#[tauri::command]
pub(crate) fn launcher_command(preset: String) -> Result<String> {
    play_arg(&["--play".into(), preset.clone()]).ok_or_else(|| Error::msg("not a preset id"))?;
    let exe = std::env::current_exe()?;
    Ok(format!("\"{}\" --play {preset}", exe.display()))
}

fn shortcut_name(name: &str) -> String {
    // (sanitize_component turns an empty name into "_")
    let cleaned = if name.trim().is_empty() { "preset".to_string() } else { crate::target::sanitize_component(name) };
    format!("GenLauncher - {cleaned}")
}

/// A shortcut on the desktop that starts the preset. Returns where it was made.
#[tauri::command]
pub(crate) async fn create_shortcut(app: AppHandle, preset: String, name: String) -> Result<String> {
    play_arg(&["--play".into(), preset.clone()]).ok_or_else(|| Error::msg("not a preset id"))?;
    let desktop = app.path().desktop_dir().map_err(|e| Error::msg(format!("Could not find your desktop: {e}")))?;
    let exe = std::env::current_exe()?;
    let lnk = desktop.join(format!("{}.lnk", shortcut_name(&name)));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // single quotes in the file name are doubled for PowerShell
        let q = |s: &str| s.replace('\'', "''");
        let script = format!(
            "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}'); $s.TargetPath='{}'; $s.Arguments='--play {preset}'; $s.IconLocation='{},0'; $s.Description='GenLauncher'; $s.Save()",
            q(&lnk.to_string_lossy()),
            q(&exe.to_string_lossy()),
            q(&exe.to_string_lossy()),
        );
        let out = tokio::task::spawn_blocking(move || {
            std::process::Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &script]).creation_flags(0x0800_0000).output()
        })
        .await
        .map_err(|e| Error::msg(e.to_string()))??;
        if !out.status.success() {
            return Err(Error::msg(format!("Could not make the shortcut: {}", String::from_utf8_lossy(&out.stderr).trim())));
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (&exe, &lnk);
        return Err(Error::msg("Shortcuts are made on Windows"));
    }
    #[cfg(windows)]
    Ok(lnk.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_play_argument_is_read_and_checked() {
        assert_eq!(play_arg(&args(&["app.exe", "--play", "p123"])).as_deref(), Some("p123"));
        assert_eq!(play_arg(&args(&["app.exe", "--minimized", "--play", "my-preset_2"])).as_deref(), Some("my-preset_2"));
        assert_eq!(play_arg(&args(&["app.exe"])), None);
        assert_eq!(play_arg(&args(&["app.exe", "--play"])), None);
        // anything that is not a plain id is refused (it ends up in a shortcut command)
        assert_eq!(play_arg(&args(&["app.exe", "--play", "a b"])), None);
        assert_eq!(play_arg(&args(&["app.exe", "--play", "x';calc;'"])), None);
        assert_eq!(play_arg(&args(&["app.exe", "--play", &"a".repeat(65)])), None);
    }

    #[test]
    fn shortcut_names_are_safe_file_names() {
        assert_eq!(shortcut_name("Contra: Hotfix / HD"), "GenLauncher - Contra_ Hotfix _ HD");
        assert_eq!(shortcut_name("   "), "GenLauncher - preset");
    }
}

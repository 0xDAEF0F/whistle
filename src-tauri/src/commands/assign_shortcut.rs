use crate::{ShortcutsConfig, get_or_create_shortcuts_config};
use std::{str::FromStr, sync::Mutex};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

#[tauri::command]
pub fn assign_shortcut(app_handle: AppHandle, name: &str, shortcut: &str) -> String {
    if name != "toggle-recording" && name != "cleanse-clipboard" {
        return "Invalid shortcut name".into();
    }

    let Ok(shortcut) = Shortcut::from_str(shortcut) else {
        return "Invalid shortcut".into();
    };

    if let Ok(old_shortcuts) = get_or_create_shortcuts_config() {
        if name == "toggle-recording" {
            _ = app_handle
                .global_shortcut()
                .unregister(old_shortcuts.toggle_recording);
        } else if name == "cleanse-clipboard" {
            _ = app_handle
                .global_shortcut()
                .unregister(old_shortcuts.cleanse_clipboard);
        }
    } else {
        return "Failed to parse shortcuts config".into();
    }

    // register the new shortcut
    _ = app_handle.global_shortcut().register(shortcut);

    // update the config
    let shortcuts_config = app_handle.state::<Mutex<ShortcutsConfig>>();
    let mut shortcuts_config = shortcuts_config.lock().unwrap();
    if name == "toggle-recording" {
        shortcuts_config.toggle_recording = shortcut;
    } else if name == "cleanse-clipboard" {
        shortcuts_config.cleanse_clipboard = shortcut;
    }

    // write the new config to disk
    let config_dir = dirs::home_dir().unwrap().join(".config/whistle/shortcuts.json");
    let file_contents = serde_json::to_string(&shortcuts_config.clone()).unwrap();
    std::fs::write(config_dir, file_contents).unwrap();

    "".into()
}

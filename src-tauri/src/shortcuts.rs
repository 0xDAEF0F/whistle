use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs::read_to_string, str::FromStr};
use tauri_plugin_global_shortcut::Shortcut;

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct ShortcutsConfig {
    pub toggle_recording: Shortcut,
    pub cleanse_clipboard: Shortcut,
}

impl Default for ShortcutsConfig {
    fn default() -> Self {
        Self {
            toggle_recording: Shortcut::from_str("CmdOrCtrl+Option+R").unwrap(),
            cleanse_clipboard: Shortcut::from_str("CmdOrCtrl+Option+C").unwrap(),
        }
    }
}

pub fn get_or_create_shortcuts_config() -> Result<ShortcutsConfig> {
    let config_path = dirs::home_dir()
        .context("Could not find home directory")?
        .join(".config/whistle/shortcuts.json");

    // create the parent directories if they don't exist
    let parent_dir = config_path.parent().context("Could not find config directory")?;
    std::fs::create_dir_all(parent_dir)?;

    if !config_path.exists() {
        let config = ShortcutsConfig::default();
        let file_contents = serde_json::to_string(&config)?;
        std::fs::write(config_path, file_contents)?;
        return Ok(config);
    }

    let file_contents = read_to_string(config_path)?;
    let config: ShortcutsConfig = serde_json::from_str(&file_contents)?;

    Ok(config)
}

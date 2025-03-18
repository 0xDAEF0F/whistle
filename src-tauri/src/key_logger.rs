use crate::{
    cleanse_clipboard,
    key_state_manager::{KeyStateManager, TranscribeAction},
    local_task_handler::Task,
    toggle_recording,
    transcribe_client::TranscribeClient,
    transcribe_icon::{Icon, TranscribeIcon},
};
use anyhow::{Context, Result};
use colored::*;
use device_query::{DeviceEvents, DeviceEventsHandler};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Manager, async_runtime::spawn};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tokio::sync::{mpsc, oneshot};

pub async fn key_logger(app_handle: AppHandle) -> Result<()> {
    let device_state = DeviceEventsHandler::new(Duration::from_millis(20))
        .context("Failed to init 'DeviceEventsHandler'")?;

    log::info!("Device state initialized");

    let key_state_manager = Arc::new(Mutex::new(KeyStateManager::new()));
    let key_state_manager_ = Arc::clone(&key_state_manager);

    // Handle key down events
    let _key_down_cb = device_state.on_key_down(move |key| {
        if !KeyStateManager::keys_in_question().contains(key) {
            log::trace!("Key pressed is not in question: {}", format!("'{key}'").blue());
            return;
        }

        let mut keys_pressed = key_state_manager.lock().unwrap();
        keys_pressed.add_key(*key);

        let Some(action) = keys_pressed.match_action() else {
            return;
        };
        drop(keys_pressed);

        if let TranscribeAction::TranscribeEnglish = action {
            toggle_recording(app_handle.clone(), true);
        } else if let TranscribeAction::CleanseTranscription = action {
            cleanse_clipboard(app_handle.clone(), true);
        }
    });

    // Handle key up events
    let _key_up_cb = device_state.on_key_up(move |key| {
        if !KeyStateManager::keys_in_question().contains(key) {
            log::trace!("Key pressed is not in question: {}", format!("'{key}'").blue());
            return;
        }
        log::trace!("Key released: {}", format!("'{key}'").blue());
        key_state_manager_.lock().unwrap().remove_key(key);
    });

    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

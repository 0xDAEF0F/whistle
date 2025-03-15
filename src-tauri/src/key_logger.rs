use crate::{
    key_state_manager::{KeyStateManager, TranscribeAction},
    local_task_handler::Task,
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
            let app_handle = app_handle.clone();
            let (tx_recording, rx_recording) = oneshot::channel::<Vec<u8>>();
            spawn(async move {
                let tx_task = app_handle.state::<mpsc::Sender<Task>>();
                tx_task
                    .send(Task::ToggleRecording(tx_recording, app_handle.clone()))
                    .await
                    .unwrap();

                let recording_bytes = rx_recording.await.unwrap();

                if recording_bytes.is_empty() {
                    return;
                }

                app_handle
                    .state::<TranscribeIcon>()
                    .change_icon(Icon::Transcribing)
                    .unwrap();

                log::debug!(
                    "Sending recording to API. Bytes: {}",
                    recording_bytes.len().to_string().yellow()
                );

                let transcribe_client = app_handle.state::<TranscribeClient>();

                let transcription = transcribe_client
                    .fetch_transcription(recording_bytes)
                    .await
                    .map_err(|e| {
                        log::error!("Failed to call API: {}", e);
                    })
                    .unwrap();

                log::info!(
                    "Writing text to clipboard: {}",
                    transcription.to_string().yellow()
                );

                app_handle
                    .clipboard()
                    .write_text(transcription)
                    .map_err(|e| {
                        log::error!("Failed to write to clipboard: {}", e);
                    })
                    .unwrap();

                log::trace!("Successfully wrote text to clipboard");

                app_handle
                    .state::<TranscribeIcon>()
                    .change_icon(Icon::Default)
                    .unwrap();

                let tx_task = app_handle.state::<mpsc::Sender<Task>>();
                tx_task.send(Task::PasteFromClipboard).await.unwrap();
            });
        } else if let TranscribeAction::CleanseTranscription = action {
            let is_cleansing_m = app_handle.state::<Arc<Mutex<bool>>>();
            let mut is_cleansing = is_cleansing_m.lock().unwrap();
            if *is_cleansing {
                log::warn!("Already cleansing. Skipping.");
                return;
            }
            *is_cleansing = true;
            drop(is_cleansing);

            _ = app_handle.state::<TranscribeIcon>().change_icon(Icon::Cleansing);

            let original_text =
                app_handle.clipboard().read_text().expect("Failed to read clipboard");
            log::info!("Starting cleanse of: {}", original_text.to_string().yellow());

            let app_handle_ = app_handle.clone();
            spawn(async move {
                let transcribe_client = app_handle_
                    .try_state::<TranscribeClient>()
                    .expect("Failed to retrieve 'TranscribeClient' (not managed)");

                let cleansed_text = transcribe_client
                    .clean_transcription(original_text)
                    .await
                    .expect("Failed to clean transcription");
                log::info!("Cleansed text: {}", cleansed_text.to_string().yellow());

                let tx_task = app_handle_.state::<mpsc::Sender<Task>>();
                let (tx_undo, rx_undo) = oneshot::channel::<()>();

                tx_task.send(Task::UndoText(tx_undo)).await.unwrap();

                let _ = rx_undo.await;

                app_handle_.clipboard().write_text(cleansed_text).unwrap();

                tx_task.send(Task::PasteFromClipboard).await.unwrap();

                _ = app_handle_.state::<TranscribeIcon>().change_icon(Icon::Default);

                let is_cleansing = app_handle_.state::<Arc<Mutex<bool>>>();
                *is_cleansing.lock().unwrap() = false;
                log::info!("Cleansing complete. Set 'IsCleansing' to false");
            });
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

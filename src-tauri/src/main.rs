#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_recorder;
mod constants;
mod enigo_instance;
mod key_logger;
mod key_state_manager;
mod local_task_handler;
mod transcribe_app_logger;
mod transcribe_client;
mod transcribe_icon;

use anyhow::Context;
use colored::*;
use key_logger::key_logger;
use local_task_handler::{Task, run_local_task_handler};
use std::sync::{Arc, Mutex};
use tauri::{
    AppHandle, Manager,
    async_runtime::spawn,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_notification::NotificationExt;
use tokio::sync::{mpsc, oneshot};
use transcribe_client::TranscribeClient;
use transcribe_icon::{Icon, TranscribeIcon};

fn main() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Debug)
                .level_for("enigo", log::LevelFilter::Error)
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Channel for sending tasks to the local task handler
            let (localtask_tx, localtask_rx) = mpsc::channel::<Task>(1);

            // Spawn a thread for the `LocalSet` to run on since
            // `Enigo` and `AudioRecorder` are not `Send` nor `Sync`
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                run_local_task_handler(localtask_rx, app_handle);
            });

            app.notification()
                .builder()
                .title("Tauri")
                .body("Tauri is awesome")
                .show()
                .unwrap();

            let toggle_recording_i = MenuItem::with_id(
                app,
                "toggle_recording",
                "Toggle Recording 🎤",
                true,
                None::<&str>,
            )?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit ✌️", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&toggle_recording_i, &quit_i])?;

            let tray_icon = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .show_menu_on_left_click(false)
                .menu(&menu)
                .build(app)?;

            let transcribe_client = TranscribeClient::new();

            app.manage(localtask_tx)
                .then(|| app.manage(transcribe_client))
                .and_then(|_| app.manage(TranscribeIcon::new(tray_icon)).into())
                .and_then(|_| app.manage(Arc::new(Mutex::new(false))).into())
                .context("Failed to manage app state")?;

            log::info!("Successfully managed app state");

            let app_handle = app.handle().clone();
            spawn(async move {
                if let Err(e) = key_logger(app_handle.clone()).await {
                    log::error!("Error on 'key_logger' task: {e}");
                    app_handle.exit(1);
                }
            });

            Ok(())
        })
        .on_tray_icon_event(|app_handle, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Right,
                button_state: MouseButtonState::Down,
                ..
            } => {
                log::info!("Tray icon right clicked");
                _ = app_handle.show_menu();
            }
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Down,
                ..
            } => {
                toggle_recording(app_handle.clone(), true);
            }
            _ => {}
        })
        .on_menu_event(|app_handle, event| match event.id.as_ref() {
            "quit" => {
                log::info!("{} application on user's request", "Quitting".red());
                app_handle.exit(0);
            }
            "toggle_recording" => {
                toggle_recording(app_handle.clone(), false);
            }
            id => {
                log::warn!("Unknown menu event: {}", id);
            }
        })
        .plugin(tauri_plugin_clipboard_manager::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

pub fn toggle_recording(app_handle: AppHandle, paste_from_clipboard: bool) {
    spawn(async move {
        let tx_task = app_handle.state::<mpsc::Sender<Task>>();
        let (tx_recording, rx_recording) = oneshot::channel::<Vec<u8>>();

        if let Err(e) = tx_task.send(Task::ToggleRecording(tx_recording)).await {
            log::error!("Failed to send 'ToggleRecording' task to channel: {}", e);
            return;
        };

        let transcribe_icon = app_handle.state::<TranscribeIcon>();

        let recording_bytes = match rx_recording.await {
            Ok(bytes) => {
                if bytes.is_empty() {
                    log::info!("Starting recording");
                    transcribe_icon.change_icon(Icon::Recording);
                    return;
                }
                bytes
            }
            Err(e) => {
                log::error!(
                    "Failed to receive 'ToggleRecording' task from channel: {}",
                    e
                );
                transcribe_icon.change_icon(Icon::Default);
                return;
            }
        };

        transcribe_icon.change_icon(Icon::Transcribing);

        let transcribe_client = app_handle.state::<TranscribeClient>();
        let result = transcribe_client.fetch_transcription(recording_bytes).await;

        transcribe_icon.change_icon(Icon::Default);

        let Ok(text) = result else {
            log::error!("Failed to fetch transcription from API");
            return;
        };

        log::info!("Transcription text: {}", text.yellow());

        if let Err(e) = app_handle.clipboard().write_text(text) {
            log::error!("Failed to write text to clipboard: {}", e);
            return;
        }

        if let Err(e) = app_handle
            .notification()
            .builder()
            .title("Done 🎉")
            .body("Your transcription is ready in your clipboard")
            .show()
        {
            log::error!("Failed to show notification: {}", e);
        }

        if !paste_from_clipboard {
            return;
        }

        if let Err(e) = tx_task.send(Task::PasteFromClipboard).await {
            log::error!("Failed to send 'PasteFromClipboard' task to channel: {}", e);
        } else {
            log::info!("Successfully pasted text from clipboard");
        }
    });
}

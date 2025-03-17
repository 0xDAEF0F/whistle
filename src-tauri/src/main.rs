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
    Manager,
    async_runtime::spawn,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_notification::NotificationExt;
use tokio::sync::{mpsc, oneshot};
use transcribe_client::TranscribeClient;
use transcribe_icon::{Icon, TranscribeIcon};

fn main() {
    // transcribe_app_logger::init(log::LevelFilter::Info); // `env_logger`
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
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
                if let Err(e) = key_logger(app_handle).await {
                    log::error!("Error on 'key_logger' task: {e}");
                };
            });

            Ok(())
        })
        .on_menu_event(|app_handle, event| match event.id.as_ref() {
            "quit" => {
                log::info!("{} application on user's request", "Quitting".red());
                app_handle.exit(0);
            }
            "toggle_recording" => {
                log::info!("{} event received", "ToggleRecording".green());
                let app_handle = app_handle.clone();
                spawn(async move {
                    log::info!("hello world!!!!");
                    let tx_task = app_handle.state::<mpsc::Sender<Task>>();
                    let (tx_recording, rx_recording) = oneshot::channel::<Vec<u8>>();
                    if let Err(e) = tx_task
                        .send(Task::ToggleRecording(tx_recording, app_handle.clone()))
                        .await
                    {
                        log::error!(
                            "Failed to send 'ToggleRecording' task to channel: {}",
                            e
                        );
                        return;
                    };

                    let recording_bytes = rx_recording.await.unwrap();

                    if recording_bytes.is_empty() {
                        log::info!("Starting recording");
                        return;
                    }

                    let transcribe_icon = app_handle.state::<TranscribeIcon>();
                    transcribe_icon.change_icon(Icon::Transcribing).unwrap();

                    let transcribe_client = app_handle.state::<TranscribeClient>();
                    let result =
                        transcribe_client.fetch_transcription(recording_bytes).await;

                    let Ok(text) = result else {
                        log::error!("Failed to fetch transcription from API");
                        return;
                    };

                    transcribe_icon.change_icon(Icon::Default).unwrap();

                    app_handle.clipboard().write_text(text).unwrap();

                    tx_task.send(Task::PasteFromClipboard).await.unwrap();
                });
            }
            id => {
                log::warn!("Unknown menu event: {}", id);
            }
        })
        .plugin(tauri_plugin_clipboard_manager::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

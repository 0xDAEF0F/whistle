#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_recorder;
mod constants;
mod enigo_instance;
mod local_task_handler;
mod notifications;
mod transcribe_app_logger;
mod transcribe_client;
mod transcribe_icon;

use anyhow::{Context, Result};
use colored::*;
use local_task_handler::{Task, run_local_task_handler};
use notifications::{AppNotifications, Notification};
use serde::{Deserialize, Serialize};
use std::fs::read_to_string;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tauri::{
    AppHandle, Manager,
    async_runtime::spawn,
    menu::{MenuBuilder, MenuItem},
    path::{BaseDirectory, PathResolver},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::{mpsc, oneshot};
use transcribe_client::TranscribeClient;
use transcribe_icon::{Icon, TranscribeIcon};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn assign_shortcut(app_handle: AppHandle, name: &str, shortcut: &str) {
    if name != "toggle-recording" && name != "cleanse-clipboard" {
        return;
    }

    let shortcut = Shortcut::from_str(shortcut).unwrap();

    if let Ok(old_shortcuts) = parse_shortcuts_config() {
        if name == "toggle-recording" {
            _ = app_handle
                .global_shortcut()
                .unregister(old_shortcuts.toggle_recording);
        } else if name == "cleanse-clipboard" {
            _ = app_handle
                .global_shortcut()
                .unregister(old_shortcuts.cleanse_clipboard);
        }
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
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
struct ShortcutsConfig {
    toggle_recording: Shortcut,
    cleanse_clipboard: Shortcut,
}

impl Default for ShortcutsConfig {
    fn default() -> Self {
        Self {
            toggle_recording: Shortcut::from_str("CmdOrCtrl+Option+R").unwrap(),
            cleanse_clipboard: Shortcut::from_str("CmdOrCtrl+Option+C").unwrap(),
        }
    }
}

fn parse_shortcuts_config() -> Result<ShortcutsConfig> {
    let config_dir = dirs::home_dir()
        .context("Could not find home directory")?
        .join(".config/whistle/shortcuts.json");
    let file_contents = read_to_string(config_dir)?;
    let config: ShortcutsConfig = serde_json::from_str(&file_contents)?;
    Ok(config)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Debug)
                .level_for("enigo", log::LevelFilter::Error)
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            #[cfg(desktop)]
            {
                let shortcuts_config = parse_shortcuts_config().unwrap_or_default();
                app.manage(Mutex::new(shortcuts_config));

                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::default()
                        .with_handler(move |app, shortcut, event| {
                            let shortcuts_config = app.state::<Mutex<ShortcutsConfig>>();
                            let shortcuts_config = shortcuts_config.lock().unwrap();

                            if event.state() == ShortcutState::Pressed {
                                log::info!("Shortcut triggered: {:?}", shortcut);
                            }

                            if shortcut == &shortcuts_config.toggle_recording
                                && event.state() == ShortcutState::Pressed
                            {
                                log::info!(
                                    "F19 shortcut triggered - Start/Stop Recording"
                                );
                                toggle_recording(app.clone(), false);
                            }
                            // Check if the shortcut matches F20
                            else if shortcut == &shortcuts_config.cleanse_clipboard
                                && event.state() == ShortcutState::Pressed
                            {
                                log::info!("F20 shortcut triggered - Polish Clipboard");
                                cleanse_clipboard(app.clone(), false);
                            }
                        })
                        .build(),
                )?;
                app.global_shortcut().register_multiple([
                    shortcuts_config.toggle_recording,
                    shortcuts_config.cleanse_clipboard,
                ])?;
                log::info!("Registered global shortcuts");
            }

            // TODO: Add activation policy for macos for app run background
            // #[cfg(target_os = "macos")]
            // app.set_activation_policy(tauri::ActivationPolicy::Accessory);

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

            let menu = MenuBuilder::new(app)
                .item(&MenuItem::with_id(
                    app,
                    "toggle_recording",
                    "Toggle Recording",
                    true,
                    None::<&str>,
                )?)
                .item(&MenuItem::with_id(
                    app,
                    "cleanse",
                    "Polish clipboard",
                    true,
                    None::<&str>,
                )?)
                .separator()
                .item(&MenuItem::with_id(
                    app,
                    "open_window",
                    "Open Window",
                    true,
                    None::<&str>,
                )?)
                .separator()
                .item(&MenuItem::with_id(app, "quit", "Quit app", true, None::<&str>)?)
                .build()?;

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

            Ok(())
        })
        .on_tray_icon_event(|app_handle, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Right,
                button_state: MouseButtonState::Down,
                ..
            } => {
                log::info!("Tray icon right clicked");
                if let Err(e) = app_handle.show_menu() {
                    log::error!("Failed to show menu: {}", e);
                }
            }
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Down,
                ..
            } => {
                toggle_recording(app_handle.clone(), false);
            }
            _ => {}
        })
        .on_window_event(|window, event| {
            log::info!("Window event received: {:?}", event);
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
                log::info!("Window close requested");
            }
        })
        .on_menu_event(|app_handle, event| {
            log::info!("Menu event received: {:?}", event.id);
            match event.id.as_ref() {
                "quit" => {
                    log::info!("{} application on user's request", "Quitting".red());
                    app_handle.exit(0);
                }
                "toggle_recording" => {
                    toggle_recording(app_handle.clone(), false);
                }
                "cleanse" => {
                    cleanse_clipboard(app_handle.clone(), false);
                }
                "open_window" => {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        if let Err(e) = window.show().and_then(|_| window.set_focus()) {
                            log::error!("Failed to show and focus window: {}", e);
                        }
                    } else {
                        log::error!("Failed to get webview window");
                    }
                }
                id => {
                    log::warn!("Unknown menu event: {}", id);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![greet, assign_shortcut])
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
            AppNotifications::new(&app_handle).notify(Notification::ApiError);
            return;
        };

        log::info!("Transcription text: {}", text.yellow());

        if let Err(e) = app_handle.clipboard().write_text(text) {
            log::error!("Failed to write text to clipboard: {}", e);
            return;
        }

        if !paste_from_clipboard {
            AppNotifications::new(&app_handle).notify(Notification::TranscribeSuccess);
            return;
        }

        if let Err(e) = tx_task.send(Task::PasteFromClipboard).await {
            log::error!("Failed to send 'PasteFromClipboard' task to channel: {}", e);
        } else {
            log::info!("Successfully pasted text from clipboard");
        }
        log::info!("exiting toggle recording function");
    });
}

pub fn cleanse_clipboard(app_handle: AppHandle, paste_from_clipboard: bool) {
    spawn(async move {
        let Ok(clipboard_text) = app_handle.clipboard().read_text() else {
            log::error!("Failed to read from clipboard");
            return;
        };

        let notifs = app_handle.notification();

        if clipboard_text.is_empty() {
            _ = notifs
                .builder()
                .title("Empty clipboard")
                .body("We couldn't find any text in your clipboard to polish")
                .show();
            return;
        }

        let is_cleansing_m = app_handle.state::<Arc<Mutex<bool>>>();
        let mut is_cleansing = is_cleansing_m.lock().unwrap();
        if *is_cleansing {
            log::warn!("Already cleansing. Skipping.");
            return;
        }
        *is_cleansing = true;
        drop(is_cleansing);

        app_handle.state::<TranscribeIcon>().change_icon(Icon::Cleansing);

        log::info!("Starting polish of: {}", clipboard_text.yellow());

        let app_handle_ = app_handle.clone();
        spawn(async move {
            let client = app_handle_.state::<TranscribeClient>();

            AppNotifications::new(&app_handle_).notify(Notification::StartPolishing);

            let Ok(cleansed_text) = client.clean_transcription(clipboard_text).await
            else {
                log::error!("Failed to clean transcription");
                AppNotifications::new(&app_handle_).notify(Notification::ApiError);
                app_handle_.state::<TranscribeIcon>().change_icon(Icon::Default);
                *app_handle_.state::<Arc<Mutex<bool>>>().lock().unwrap() = false;
                return;
            };

            log::info!("Polished text: {}", cleansed_text.to_string().yellow());

            app_handle_.clipboard().write_text(cleansed_text).unwrap();

            if !paste_from_clipboard {
                AppNotifications::new(&app_handle_).notify(Notification::PolishSuccess);
                *app_handle_.state::<Arc<Mutex<bool>>>().lock().unwrap() = false;
                app_handle_.state::<TranscribeIcon>().change_icon(Icon::Default);
                return;
            }

            let tx_task = app_handle_.state::<mpsc::Sender<Task>>();
            let (tx_undo, rx_undo) = oneshot::channel::<()>();

            tx_task.send(Task::UndoText(tx_undo)).await.unwrap();

            _ = rx_undo.await; // Wait for the undo future to complete

            tx_task.send(Task::PasteFromClipboard).await.unwrap();

            app_handle_.state::<TranscribeIcon>().change_icon(Icon::Default);

            let is_cleansing = app_handle_.state::<Arc<Mutex<bool>>>();
            *is_cleansing.lock().unwrap() = false;

            log::info!("Cleansing complete. Set 'IsCleansing' to false");
        });
    });
}

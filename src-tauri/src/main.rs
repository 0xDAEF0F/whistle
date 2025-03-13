#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_recorder;
mod constants;
mod debouncer;
mod key_state_manager;
mod transcribe_app_logger;
mod transcribe_client;
use anyhow::{Context, Result};
use audio_recorder::AudioRecorder;
use colored::*;
use device_query::{DeviceEvents, DeviceEventsHandler};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use key_state_manager::{KeyStateManager, TranscribeAction};
use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{
    AppHandle, Manager,
    async_runtime::spawn,
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use transcribe_client::TranscribeClient;

thread_local! {
    static RECORDER: RefCell<AudioRecorder> = RefCell::new(AudioRecorder::new());
    static ENIGO: RefCell<Enigo> = RefCell::new(Enigo::new(&Settings::default()).expect("Failed to create Enigo"));
}

#[derive(Debug, Clone, Copy)]
pub enum Icon {
    Default,
    Recording,
    Transcribing,
    Cleansing,
}

pub struct TranscribeIcon(TrayIcon);

impl TranscribeIcon {
    pub fn new(tray_icon: TrayIcon) -> Self {
        Self(tray_icon)
    }

    pub fn change_icon(&self, icon: Icon) -> Result<()> {
        let img = match icon {
            Icon::Default => Image::from_path("icons/StoreLogo.png")?, // TODO
            Icon::Recording => Image::from_path("icons/recording-icon.png")?,
            Icon::Transcribing => Image::from_path("icons/transcribing-icon.png")?,
            Icon::Cleansing => Image::from_path("icons/transcribing-icon.png")?, // TODO
        };

        self.0.set_icon(Some(img))?;

        Ok(())
    }
}

pub fn main() {
    transcribe_app_logger::init(log::LevelFilter::Info);

    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit_i])?;

            let tray_icon = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .build(app)?;

            let transcribe_client = TranscribeClient::new();

            let is_success = app.manage(transcribe_client);
            assert!(is_success, "Failed to manage 'TranscribeClient'");
            log::info!("Successfully managed 'TranscribeClient'");

            let is_success = app.manage(TranscribeIcon::new(tray_icon));
            assert!(is_success, "Failed to manage 'TranscribeIcon'");
            log::info!("Successfully managed 'TranscribeIcon'");

            let is_success = app.manage(Arc::new(Mutex::new(false)));
            assert!(is_success, "Failed to manage 'IsCleansing'");
            log::info!("Successfully managed 'IsCleansing'");

            let app_handle = app.handle().clone();
            spawn(async move {
                if let Err(e) = key_logger(app_handle).await {
                    log::error!("Error on 'key_logger' task: {e}");
                };
            });

            Ok(())
        })
        .on_tray_icon_event(|app_handle, event| {
            if let TrayIconEvent::Click {
                button_state: MouseButtonState::Down,
                ..
            } = event
            {
                log::trace!(
                    "Tray icon clicked at: {}",
                    chrono::Local::now().format("%H:%M%p").to_string().yellow()
                );
                let recording_result =
                    toggle_recording().expect("Failed to toggle recording");

                if let RecordingResult::StartRecording = recording_result {
                    _ = app_handle.state::<TranscribeIcon>().change_icon(Icon::Recording);
                }
                if let RecordingResult::RecordingResult(recording_bytes) =
                    recording_result
                {
                    let app_handle = app_handle.clone();
                    _ = app_handle
                        .state::<TranscribeIcon>()
                        .change_icon(Icon::Transcribing);
                    spawn(async move {
                        let transcribe_client =
                            app_handle.try_state::<TranscribeClient>();
                        let transcribe_client = transcribe_client
                            .context(
                                "Failed to retrieve 'TranscribeClient' (not managed)",
                            )
                            .map_err(|e| {
                                log::error!("{e}");
                                e
                            })?;
                        let text = transcribe_client
                            .fetch_transcription(recording_bytes)
                            .await
                            .map_err(|e| {
                                log::error!("Failed to call the API: {}", e);
                                e
                            })?;

                        log::info!(
                            "Writing text to clipboard: {}",
                            text.to_string().yellow()
                        );

                        app_handle
                            .clipboard()
                            .write_text(text)
                            .map_err(|e| {
                                log::error!("Failed to write to clipboard: {}", e);
                            })
                            .unwrap();
                        log::trace!("Successfully wrote text to clipboard");

                        _ = app_handle
                            .state::<TranscribeIcon>()
                            .change_icon(Icon::Default);

                        log::trace!("Successfully set tray icon to default");
                        anyhow::Ok(())
                    });
                }
            }
        })
        .on_menu_event(|app_handle, event| match event.id.as_ref() {
            "quit" => app_handle.exit(0),
            id => {
                log::warn!("Unknown menu event: {}", id);
            }
        })
        .plugin(tauri_plugin_clipboard_manager::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn key_logger(app_handle: AppHandle) -> Result<()> {
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

            _ = app_handle.state::<TranscribeIcon>().change_icon(Icon::Recording);

            let rec_res = toggle_recording().map_err(|e| {
                log::error!("Failed to toggle recording: {}", e);
            });
            let Ok(RecordingResult::RecordingResult(recording_bytes)) = rec_res else {
                return;
            };

            _ = app_handle.state::<TranscribeIcon>().change_icon(Icon::Transcribing);

            log::debug!(
                "Sending recording to API. Bytes: {}",
                recording_bytes.len().to_string().yellow()
            );

            // Do not block the UI thread.
            spawn(async move {
                let transcribe_client = app_handle.try_state::<TranscribeClient>();
                let transcribe_client = transcribe_client
                    .context("Failed to retrieve 'TranscribeClient' (not managed)")
                    .map_err(|e| {
                        log::error!("{e}");
                        e
                    })
                    .unwrap(); // TODO: Handle this better.

                let text = transcribe_client
                    .fetch_transcription(recording_bytes)
                    .await
                    .map_err(|e| {
                        log::error!("Failed to call API: {}", e);
                    })
                    .unwrap();

                log::info!("Writing text to clipboard: {}", text.to_string().yellow());

                app_handle
                    .clipboard()
                    .write_text(text)
                    .map_err(|e| {
                        log::error!("Failed to write to clipboard: {}", e);
                    })
                    .unwrap();

                log::trace!("Successfully wrote text to clipboard");

                _ = app_handle.state::<TranscribeIcon>().change_icon(Icon::Default);

                ENIGO.with_borrow_mut(|enigo| {
                    _ = enigo.key(Key::Meta, Direction::Press);
                    _ = enigo.key(Key::Unicode('v'), Direction::Click);
                    _ = enigo.key(Key::Meta, Direction::Release);
                });
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

                ENIGO.with_borrow_mut(|enigo| {
                    _ = enigo.key(Key::Meta, Direction::Press);
                    _ = enigo.key(Key::Unicode('z'), Direction::Click);

                    app_handle_
                        .clipboard()
                        .write_text(cleansed_text)
                        .expect("Failed to write to clipboard");

                    _ = enigo.key(Key::Unicode('v'), Direction::Click);
                    _ = enigo.key(Key::Meta, Direction::Release);
                });
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

enum RecordingResult {
    RecordingResult(Vec<u8>),
    StartRecording,
}

/// This function:
/// - Pauses Spotify if it is playing.
/// - Starts recording or stops recording.
fn toggle_recording() -> Result<RecordingResult> {
    let recording_result = RECORDER.with_borrow_mut(|recorder| {
        log::info!(
            "Recorder is recording: {}",
            recorder.is_recording.to_string().yellow()
        );

        if !recorder.is_recording {
            _ = std::process::Command::new("osascript")
                .args(["-e", "tell application \"Spotify\" to pause"])
                .output();

            recorder.start_recording();
            return anyhow::Ok(RecordingResult::StartRecording);
        }

        let recording_bytes = recorder
            .stop_recording_and_get_bytes()
            .context("Failed to stop recording")?;

        Ok(RecordingResult::RecordingResult(recording_bytes))
    })?;

    Ok(recording_result)
}

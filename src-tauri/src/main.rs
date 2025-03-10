#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod constants;
use anyhow::{Context, Result};
use colored::*;
use constants::API_BASE_URL;
use cpal::{
    Stream,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use device_query::{DeviceEvents, DeviceEventsHandler, Keycode};
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use env_logger::WriteStyle;
use hound::{WavSpec, WavWriter};
use reqwest::Client;
use serde::Deserialize;
use std::{
    cell::RefCell,
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{
    AppHandle, Manager,
    async_runtime::spawn,
    image::Image,
    tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent, TrayIconId},
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tempfile::NamedTempFile;

struct AudioRecorder {
    stream: Option<Stream>,
    sample_rate: Option<u32>,
    channels: Option<u16>,
    samples: Arc<Mutex<Vec<i16>>>,
    is_recording: bool,
}

impl AudioRecorder {
    fn new() -> Self {
        Self {
            stream: None,
            sample_rate: None,
            channels: None,
            samples: Arc::new(Mutex::new(Vec::new())),
            is_recording: false,
        }
    }

    fn start_recording(&mut self) {
        if self.is_recording {
            return;
        }

        log::debug!("'AudioRecorder' starting to record!");

        let device = cpal::default_host()
            .default_input_device()
            .expect("No input device available");
        let config = device.default_input_config().unwrap();

        // Store audio format information
        self.sample_rate = Some(config.sample_rate().0);
        self.channels = Some(config.channels() as u16);

        // Clear previous samples
        self.samples.lock().unwrap().clear();

        // Create a samples buffer for the callback
        let samples_for_callback = self.samples.clone();

        self.is_recording = true;
        log::debug!(
            "'AudioRecorder' is recording: {} (should be true)",
            self.is_recording
        );

        let stream = device
            .build_input_stream(
                &config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let mut samples = samples_for_callback.lock().unwrap();
                    for &sample in data {
                        // Apply gain (increase volume) - adjust the multiplier as needed
                        let amplified_sample = sample * 3.0;

                        // Avoids distortion
                        let clamped_sample = amplified_sample.clamp(-1.0, 1.0);

                        // Convert f32 to i16
                        let sample = (clamped_sample * 32767.0) as i16;
                        samples.push(sample);
                    }
                },
                |err| log::error!("An error occurred on the audio stream: {}", err),
                None,
            )
            .expect("Failed to build input stream");

        stream.play().expect("Failed to start audio stream");
        self.stream = Some(stream);
    }

    fn stop_recording_and_get_bytes(&mut self) -> Option<Vec<u8>> {
        if !self.is_recording {
            return None;
        }

        log::debug!("'AudioRecorder' stopping recording");

        self.is_recording = false;
        log::debug!(
            "'AudioRecorder' is recording: {} (should be false)",
            self.is_recording
        );

        // Drop the stream to stop recording
        self.stream = None;

        // Get the recorded samples
        let samples = self.samples.lock().unwrap().clone();

        if samples.is_empty() || self.sample_rate.is_none() || self.channels.is_none() {
            return None;
        }

        // Create a temporary file for the WAV
        let temp_file = match NamedTempFile::new() {
            Ok(file) => file,
            Err(e) => {
                eprintln!("Error creating temporary file: {}", e);
                return None;
            }
        };

        let temp_path = temp_file.path().to_owned();

        // Create a WAV file
        let spec = WavSpec {
            channels: self.channels.unwrap(),
            sample_rate: self.sample_rate.unwrap(),
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        // Create a writer for the WAV file
        let mut writer = match WavWriter::create(&temp_path, spec) {
            Ok(writer) => writer,
            Err(e) => {
                eprintln!("Error creating WAV writer: {}", e);
                return None;
            }
        };

        // Write all samples
        for &sample in &samples {
            if let Err(e) = writer.write_sample(sample) {
                eprintln!("Error writing sample: {}", e);
                return None;
            }
        }

        // Finalize the WAV file
        if let Err(e) = writer.finalize() {
            eprintln!("Error finalizing WAV file: {}", e);
            return None;
        }

        // Read the file back into memory
        match std::fs::read(&temp_path) {
            Ok(bytes) => {
                let size_mb = bytes.len() as f64 / 1_048_576.0;
                let formatted_size = format!("{:.2} MB", size_mb);
                log::info!("Recording captured: {}", formatted_size.red());
                Some(bytes)
            }
            Err(e) => {
                log::error!("Error reading WAV file: {}", e);
                None
            }
        }
    }
}

thread_local! {
    static RECORDER: RefCell<AudioRecorder> = RefCell::new(AudioRecorder::new());
    static ENIGO: RefCell<Enigo> = RefCell::new(Enigo::new(&Settings::default()).expect("Failed to create Enigo"));
}

pub fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format(|buf, record| {
            use std::io::Write;
            let timestamp = chrono::Local::now().format("%I:%M%p");
            let style = buf.default_level_style(record.level());
            let level_style = format!("{style}{}{style:#}", record.level());
            writeln!(
                buf,
                "[{} {} {}] {}",
                timestamp,
                level_style,
                record.target(),
                record.args()
            )
        })
        .format_level(true)
        .write_style(WriteStyle::Always)
        .init();

    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let tray_icon = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .build(app)?;

            let transcribe_client = TranscribeClient::new();
            if !app.handle().manage(transcribe_client) {
                log::error!("Failed to manage 'TranscribeClient'");
            } else {
                log::info!("Successfully managed 'TranscribeClient'");
            };

            spawn(key_logger(app.handle().clone(), tray_icon.id().clone()));

            Ok(())
        })
        .on_tray_icon_event(|app_handle, event| {
            if let TrayIconEvent::Click {
                id: tray_id,
                button_state: MouseButtonState::Down,
                ..
            } = event
            {
                log::debug!(
                    "Tray icon clicked at: {}",
                    chrono::Local::now().format("%H:%M%p").to_string().yellow()
                );
                let recording_result =
                    toggle_recording(app_handle.clone(), tray_id.clone())
                        .expect("Failed to toggle recording");

                if let RecordingResult::RecordingResult(recording_bytes) =
                    recording_result
                {
                    let app_handle = app_handle.clone();
                    let tray_id = tray_id.clone();
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

                        _ = app_handle.tray_by_id(&tray_id).unwrap().set_icon(Some(
                            app_handle.default_window_icon().unwrap().clone(),
                        ));
                        log::trace!("Successfully set tray icon to default");
                        anyhow::Ok(())
                    });
                }
            }
        })
        .plugin(tauri_plugin_clipboard_manager::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn key_logger(app_handle: AppHandle, tray_id: TrayIconId) -> Result<()> {
    let device_state = DeviceEventsHandler::new(Duration::from_millis(20))
        .expect("Failed to start event loop");

    struct KeysPressed(HashSet<Keycode>);

    enum TranscribeAction {
        TranscribeEnglish,
        TranscribeSpanish,
        CleanseTranscription, // from clipboard
    }

    impl KeysPressed {
        pub fn new() -> Self {
            Self(HashSet::new())
        }

        pub fn add_key(&mut self, key: Keycode) {
            self.0.insert(key);
        }

        pub fn remove_key(&mut self, key: &Keycode) {
            self.0.remove(key);
        }

        pub fn match_action(&self) -> Option<TranscribeAction> {
            use Keycode::*;
            if self.0.is_superset(&[F19].into()) {
                return Some(TranscribeAction::TranscribeEnglish);
            }
            if self.0.is_superset(&[F20].into()) {
                return Some(TranscribeAction::CleanseTranscription);
            }
            None
        }

        pub fn keys_in_question() -> [Keycode; 2] {
            use Keycode::*;
            [F19, F20]
        }
    }

    let keys_pressed = Arc::new(Mutex::new(KeysPressed::new()));
    let keys_pressed_ = Arc::clone(&keys_pressed);

    // Handle key down events
    let _key_down_cb = device_state.on_key_down(move |&key| {
        if !KeysPressed::keys_in_question().contains(&key) {
            log::trace!("Key pressed is not in question: {}", format!("'{key}'").blue());
            return;
        }

        let mut keys_pressed = keys_pressed.lock().unwrap();
        keys_pressed.add_key(key);

        let Some(action) = keys_pressed.match_action() else {
            return;
        };
        drop(keys_pressed);

        if let TranscribeAction::TranscribeEnglish = action {
            let rec_res =
                toggle_recording(app_handle.clone(), tray_id.clone()).map_err(|e| {
                    log::error!("Failed to toggle recording: {}", e);
                });
            let Ok(RecordingResult::RecordingResult(recording_bytes)) = rec_res else {
                return;
            };

            log::debug!(
                "Sending recording to API. Bytes: {}",
                recording_bytes.len().to_string().yellow()
            );

            // Do not block the UI thread.
            let app_handle = app_handle.clone();
            let tray_id = tray_id.clone();
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

                _ = app_handle
                    .tray_by_id(&tray_id)
                    .unwrap()
                    .set_icon(Some(app_handle.default_window_icon().unwrap().clone()));

                ENIGO.with_borrow_mut(|enigo| {
                    _ = enigo.key(Key::Meta, Direction::Press);
                    _ = enigo.key(Key::Unicode('v'), Direction::Click);
                    _ = enigo.key(Key::Meta, Direction::Release);
                });
            });
        } else if let TranscribeAction::CleanseTranscription = action {
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
                })
            });
        }
    });

    // Handle key up events
    let _key_up_cb = device_state.on_key_up(move |key| {
        if !KeysPressed::keys_in_question().contains(key) {
            log::trace!("Key pressed is not in question: {}", format!("'{key}'").blue());
            return;
        }
        log::trace!("Key released: {}", format!("'{key}'").blue());
        keys_pressed_.lock().unwrap().remove_key(key);
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
/// - Changes the tray icon.
/// - Pauses Spotify if it is playing.
/// - Starts recording or stops recording.
fn toggle_recording(
    app_handle: AppHandle,
    tray_id: TrayIconId,
) -> Result<RecordingResult> {
    let tray_icon = app_handle
        .tray_by_id(&tray_id)
        .with_context(|| format!("could not get tray_icon from tray_id: {tray_id:?}"))?;

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
            tray_icon.set_icon(Some(Image::from_path("icons/recording-icon.png")?))?;
            return anyhow::Ok(RecordingResult::StartRecording);
        }

        let recording_bytes = recorder
            .stop_recording_and_get_bytes()
            .context("Failed to stop recording")?;

        tray_icon.set_icon(Some(Image::from_path("icons/transcribing-icon.png")?))?;

        Ok(RecordingResult::RecordingResult(recording_bytes))
    })?;

    Ok(recording_result)
}

struct TranscribeClient {
    http_client: Client,
}

#[derive(Debug, Deserialize)]
struct TranscriptionResponse {
    text: String,
    original_text: Option<String>,
}

impl TranscribeClient {
    fn new() -> Self {
        Self {
            http_client: Client::new(),
        }
    }

    async fn fetch_transcription(&self, recording: Vec<u8>) -> Result<String> {
        let res = self
            .http_client
            .post(format!("{API_BASE_URL}/transcribe"))
            .header("Content-Type", "audio/wav")
            .body(recording)
            .send()
            .await?;

        let res: TranscriptionResponse = res.json().await?;

        Ok(res.text)
    }

    async fn clean_transcription(&self, transcription: String) -> Result<String> {
        let res = self
            .http_client
            .post(format!("{API_BASE_URL}/clean-transcription"))
            .header("Content-Type", "application/json")
            .body(serde_json::json!({ "text": transcription }).to_string())
            .send()
            .await?;

        let response: TranscriptionResponse = res.json().await?;

        let _original_text =
            response.original_text.context("Failed to get original text")?;

        Ok(response.text)
    }
}

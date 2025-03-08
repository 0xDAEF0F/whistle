#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod constants;
use anyhow::{Context, Result};
use colored::*;
use constants::API_URL;
use cpal::{
    Stream,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use device_query::{DeviceEvents, DeviceEventsHandler, Keycode};
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
    AppHandle,
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

        let err_fn = |err| eprintln!("an error occurred on the audio stream: {}", err);

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
                err_fn,
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
                println!("Recording captured ({} bytes)", bytes.len());
                Some(bytes)
            }
            Err(e) => {
                eprintln!("Error reading WAV file: {}", e);
                None
            }
        }
    }
}

thread_local! {
    static RECORDER: RefCell<AudioRecorder> = RefCell::new(AudioRecorder::new());
}

pub fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .write_style(WriteStyle::Always)
        .format_timestamp(None)
        .init();
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let tray_icon = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .build(app)?;

            let app_handle = app.handle().clone();
            let tray_id = tray_icon.id().clone();

            _ = spawn(key_logger(app_handle, tray_id));

            Ok(())
        })
        .on_tray_icon_event(|app_handle, event| {
            if let TrayIconEvent::Click {
                id: tray_id,
                button_state: MouseButtonState::Down,
                ..
            } = event
            {
                log::info!("Tray icon clicked at: {}", chrono::Local::now());
                let tray_id = tray_id.clone();
                let recording_result = toggle_recording(app_handle.clone(), tray_id)
                    .expect("Failed to toggle recording");

                if let RecordingResult::RecordingResult(recording_bytes) =
                    recording_result
                {
                    let app_handle = app_handle.clone();
                    spawn(async move {
                        let text = call_api_and_retrieve_transcription(
                            reqwest::Client::new(),
                            recording_bytes,
                            Language::English,
                        )
                        .await
                        .map_err(|e| {
                            log::error!("Failed to call API: {}", e);
                        })
                        .unwrap();

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
                    });
                }
            }
        })
        .plugin(tauri_plugin_clipboard_manager::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn key_logger(app_handle: AppHandle, tray_id: TrayIconId) -> Result<()> {
    let device_state = DeviceEventsHandler::new(Duration::from_millis(10))
        .expect("Failed to start event loop");

    let modifiers_held = Arc::new(Mutex::new(HashSet::new()));
    let modifiers_held_ = Arc::clone(&modifiers_held);

    // Handle key down events
    let _key_down_cb = device_state.on_key_down(move |&key| {
        if key == Keycode::Command || key == Keycode::LOption || key == Keycode::ROption {
            log::debug!("Modifier key pressed: {}", key);
            modifiers_held.lock().unwrap().insert(key);
            return;
        }

        {
            let modifiers_held_ = modifiers_held.lock().unwrap();
            if !(modifiers_held_.contains(&Keycode::Command)
                && (modifiers_held_.contains(&Keycode::LOption)
                    || modifiers_held_.contains(&Keycode::ROption)))
            {
                log::debug!("Key pressed '{key}' while modifiers not held. Returning...");
                return;
            }
        }

        if key != Keycode::R && key != Keycode::S {
            return;
        }

        log::info!(
            "'{}' pressed while modifiers held. {} recording...",
            key.to_string().red(),
            "Toggling".red()
        );

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

        let language = if key == Keycode::S {
            Language::Spanish
        } else {
            Language::English
        };

        // Do not block the UI thread.
        let app_handle = app_handle.clone();
        spawn(async move {
            let text = call_api_and_retrieve_transcription(
                reqwest::Client::new(),
                recording_bytes,
                language,
            )
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
        });
    });

    // Handle key up events
    let _key_up_cb = device_state.on_key_up(move |&key| {
        if key == Keycode::Command || key == Keycode::LOption || key == Keycode::ROption {
            log::debug!("modifier key released: {}", key);
            modifiers_held_.lock().unwrap().remove(&key);
        }
    });

    loop {
        tokio::time::sleep(Duration::from_millis(10)).await;
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
        log::info!("Recorder is recording: {}", recorder.is_recording);

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

        Ok(RecordingResult::RecordingResult(recording_bytes))
    })?;

    Ok(recording_result)
}

#[derive(Default, Debug)]
enum Language {
    #[default]
    English,
    Spanish,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

async fn call_api_and_retrieve_transcription(
    http_client: Client,
    recording: Vec<u8>,
    language: Language,
) -> Result<String> {
    let lang = language.to_string();
    let res = http_client
        .post(API_URL)
        .header("Content-Type", "audio/wav")
        .query(&[("lang", lang.as_str()), ("model", "small")])
        .body(recording)
        .send()
        .await?;

    #[derive(Deserialize)]
    struct Response {
        text: String,
    }
    let response: Response = serde_json::from_str(&res.text().await?)?;

    Ok(response.text)
}

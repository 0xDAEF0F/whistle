#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod constants;
use anyhow::{Context, Result};
use chrono::naive;
use constants::API_URL;
use cpal::{
    Stream,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use device_query::{DeviceEvents, DeviceEventsHandler, DeviceState, Keycode};
use hound::{WavSpec, WavWriter};
use serde::Deserialize;
use std::{
    cell::RefCell,
    collections::HashSet,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};
use tauri::{
    AppHandle, Manager, State,
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
        log::debug!("Starting to record!");

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

        println!("Recording started");
    }

    fn stop_recording_and_get_bytes(&mut self) -> Option<Vec<u8>> {
        if !self.is_recording {
            return None;
        }
        log::debug!("Stopping recording");

        self.is_recording = false;

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

#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct KeyboardShortcut {
    modifiers: HashSet<Keycode>,
    action_key: Option<Keycode>,
}

impl KeyboardShortcut {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_key(&mut self, key: Keycode) {
        match key {
            Keycode::Command | Keycode::LOption | Keycode::ROption => {
                self.modifiers.insert(key);
            }
            _ => self.action_key = Some(key),
        }
    }
}

pub enum ShortcutAction {
    ToggleRecording,
    ToggleRecordingSpanish,
}

thread_local! {
    static RECORDER: RefCell<AudioRecorder> = RefCell::new(AudioRecorder::new());
}

pub fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .format_timestamp(None)
        .init();
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            app.manage(reqwest::Client::new());

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
                spawn(toggle_recording(app_handle.app_handle().clone(), tray_id));
            }
        })
        .plugin(tauri_plugin_clipboard_manager::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn key_logger(app_handle: AppHandle, tray_id: TrayIconId) -> Result<()> {
    let device_state = DeviceEventsHandler::new(Duration::from_millis(10))
        .expect("Failed to start event loop");

    let seq_of_keys = Arc::new(Mutex::new((false, false)));
    let seq_of_keys_ = Arc::clone(&seq_of_keys);

    // Handle key down events
    let _on_key_down_cb = device_state.on_key_down(move |&key| {
        if key == Keycode::Command {
            seq_of_keys.lock().unwrap().0 = true;
            log::info!("command key pressed");
            return;
        }
        if key == Keycode::LOption || key == Keycode::ROption {
            seq_of_keys.lock().unwrap().1 = true;
            log::info!("option key pressed");
            return;
        }

        let (cmd_pressed, option_pressed) = *seq_of_keys.lock().unwrap();
        if cmd_pressed && option_pressed && key == Keycode::R {
            log::info!("should toggle recording");

            let is_recording = RECORDER.with(|recorder| recorder.borrow().is_recording);
            if !is_recording {
                _ = RECORDER.with(|recorder| {
                    let tray_icon =
                        app_handle.tray_by_id(&tray_id).with_context(|| {
                            format!("could not get tray_icon from tray_id: {tray_id:?}")
                        })?;

                    let _ = std::process::Command::new("osascript")
                        .args(["-e", "tell application \"Spotify\" to pause"])
                        .output();

                    recorder.borrow_mut().start_recording();

                    tray_icon
                        .set_icon(Some(Image::from_path("icons/recording-icon.png")?))?;

                    anyhow::Ok(())
                });
            } else {
                let recording_bytes = RECORDER
                    .with(|recorder| {
                        let mut recorder = recorder.borrow_mut();
                        recorder
                            .stop_recording_and_get_bytes()
                            .context("Failed to stop recording")
                    })
                    .unwrap();
                let tray_icon = app_handle
                    .tray_by_id(&tray_id)
                    .with_context(|| {
                        format!("could not get tray_icon from tray_id: {tray_id:?}")
                    })
                    .unwrap();
                _ = tray_icon.set_icon(Some(
                    Image::from_path("icons/transcribing-icon.png").unwrap(),
                ));

                #[derive(Deserialize)]
                struct Response {
                    text: String,
                }

                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    async fn call_api_and_retrieve_transcription(
                        http_client: State<'_, reqwest::Client>,
                        recording: Vec<u8>,
                    ) -> Result<String> {
                        let res = http_client
                            .post(API_URL)
                            .header("Content-Type", "audio/wav")
                            .body(recording)
                            .send()
                            .await?;

                        let response: Response =
                            serde_json::from_str(&res.text().await?)?;

                        Ok(response.text)
                    }

                    let transcription = call_api_and_retrieve_transcription(
                        app_handle.state::<reqwest::Client>(),
                        recording_bytes,
                    )
                    .await;

                    match transcription {
                        std::result::Result::Ok(text) => {
                            log::info!("Transcription success: {}", text);
                            app_handle.clipboard().write_text(text)?;
                            tray_icon
                                .set_icon(Some(Image::from_path("icons/icon.png")?))?;
                        }
                        Err(e) => {
                            log::error!("Error writing to clipboard: {}", e);
                            tray_icon
                                .set_icon(Some(Image::from_path("icons/icon.png")?))?;
                            let default_icon = app_handle.default_window_icon().unwrap();
                            tray_icon.set_icon(Some(default_icon.clone()))?;
                        }
                    }
                    anyhow::Ok(())
                });
            }
            *seq_of_keys.lock().unwrap() = (false, false);
        }
    });

    // Handle key up events
    let _on_key_up_cb = device_state.on_key_up(move |&key| {
        if key == Keycode::Command {
            seq_of_keys_.lock().unwrap().0 = false;
            log::info!("command key released");
        }
        if key == Keycode::LOption || key == Keycode::ROption {
            seq_of_keys_.lock().unwrap().1 = false;
            log::info!("option key released");
        }
    });

    loop {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// This function:
/// - Changes the tray icon.
/// - Pauses Spotify if it is playing.
/// - Starts recording or stops recording.
/// - Calls the API to transcribe the recording.
async fn toggle_recording(app_handle: AppHandle, tray_id: TrayIconId) -> Result<()> {
    let tray_icon = app_handle
        .tray_by_id(&tray_id)
        .with_context(|| format!("could not get tray_icon from tray_id: {tray_id:?}"))?;

    let is_recording = RECORDER.with_borrow(|recorder| recorder.is_recording);

    if !is_recording {
        return RECORDER.with_borrow_mut(|recorder| {
            _ = std::process::Command::new("osascript")
                .args(["-e", "tell application \"Spotify\" to pause"])
                .output();

            recorder.start_recording();

            tray_icon.set_icon(Some(Image::from_path("icons/recording-icon.png")?))?;

            Ok(())
        });
    }

    let recording_bytes = RECORDER
        .with_borrow_mut(|recorder| recorder.stop_recording_and_get_bytes())
        .context("Failed to stop recording")?;

    let transcription = call_api_and_retrieve_transcription(
        app_handle.state::<reqwest::Client>(),
        recording_bytes,
        None,
    )
    .await;

    match transcription {
        Ok(text) => {
            log::info!("Transcription success: {}", text);
            app_handle.clipboard().write_text(text)?;
            tray_icon.set_icon(Some(Image::from_path("icons/icon.png")?))?;
        }
        Err(e) => {
            log::error!("Error writing to clipboard: {}", e);
            tray_icon.set_icon(Some(Image::from_path("icons/icon.png")?))?;
            let default_icon = app_handle.default_window_icon().unwrap();
            tray_icon.set_icon(Some(default_icon.clone()))?;
        }
    }
    Ok(())
}

async fn call_api_and_retrieve_transcription(
    http_client: State<'_, reqwest::Client>,
    recording: Vec<u8>,
    language: Option<&str>, // English or Spanish (default English)
) -> Result<String> {
    let lang = language.unwrap_or("English");
    if lang != "English" && lang != "Spanish" {
        anyhow::bail!("Invalid language!");
    }
    let res = http_client
        .post(API_URL)
        .header("Content-Type", "audio/wav")
        .query(&["lang", lang])
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

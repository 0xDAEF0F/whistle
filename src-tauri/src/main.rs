// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{Context, Result};
use cpal::Stream;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{WavSpec, WavWriter};
use serde::Deserialize;
use std::cell::RefCell;
use std::fs;
use std::sync::{Arc, Mutex};
use tauri::image::Image;
use tauri::tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent, TrayIconId};
use tauri::{AppHandle, Manager};
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
        match fs::read(&temp_path) {
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
        .format_timestamp(None)
        .init();
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            app.manage(reqwest::Client::new());

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .build(app)?;

            Ok(())
        })
        .on_tray_icon_event(|app_handle, event| {
            if let Err(e) = tray_event_handler(app_handle, event) {
                log::error!("Error handling tray event: {}", e);
            }
        })
        .plugin(tauri_plugin_clipboard_manager::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn tray_event_handler(app_handle: &AppHandle, event: TrayIconEvent) -> Result<()> {
    if let TrayIconEvent::Click {
        id: tray_id,
        button_state: MouseButtonState::Down,
        ..
    } = event
    {
        let is_recording = RECORDER.with(|recorder| recorder.borrow().is_recording);
        match is_recording {
            false => {
                _ = RECORDER.with(|recorder| {
                    let tray_icon =
                        app_handle.tray_by_id(&tray_id).with_context(|| {
                            format!("could not get tray_icon from tray_id: {tray_id:?}")
                        })?;

                    recorder.borrow_mut().start_recording();
                    tray_icon
                        .set_icon(Some(Image::from_path("icons/recording-icon.png")?))?;

                    anyhow::Ok(())
                });
            }
            true => {
                let recording_bytes = RECORDER.with(|recorder| {
                    let mut recorder = recorder.borrow_mut();
                    recorder
                        .stop_recording_and_get_bytes()
                        .context("Failed to stop recording")
                })?;
                let tray_icon = app_handle.tray_by_id(&tray_id).with_context(|| {
                    format!("could not get tray_icon from tray_id: {tray_id:?}")
                })?;
                tray_icon
                    .set_icon(Some(Image::from_path("icons/transcribing-icon.png")?))?;

                #[derive(Deserialize)]
                struct Response {
                    text: String,
                }

                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let response = app_handle
                        .state::<reqwest::Client>()
                        .post("http://localhost:3000/upload-wav")
                        .header("Content-Type", "audio/wav")
                        .body(recording_bytes)
                        .send()
                        .await?;
                    let response: Response =
                        serde_json::from_str(&response.text().await?)?;

                    app_handle.clipboard().write_text(response.text)?;
                    tray_icon.set_icon(Some(Image::from_path("icons/icon.png")?))?;

                    anyhow::Ok(())
                });
            }
        }
    }

    Ok(())
}

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Stream;
use hound::{WavSpec, WavWriter};
use std::fs::File;
use std::io::BufWriter;
use std::sync::{Arc, Mutex};
use tauri::tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Listener};

struct AudioRecorder {
    stream: Option<Stream>,
    writer: Option<WavWriter<BufWriter<File>>>,
    is_recording: bool,
}

impl AudioRecorder {
    fn new() -> Self {
        Self {
            stream: None,
            writer: None,
            is_recording: false,
        }
    }

    fn start_recording(&mut self) {
        if self.is_recording {
            return; // Already recording
        }

        // Configure audio host and input device
        let host = cpal::default_host();
        let device = host.default_input_device().expect("No input device available");
        let config = device.default_input_config().unwrap();

        let filename = "recording.wav".to_string();

        // Configure WAV file
        let spec = WavSpec {
            channels: config.channels() as u16,
            sample_rate: config.sample_rate().0,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        // Create a writer for the recording
        let writer = WavWriter::create(&filename, spec).unwrap();
        self.writer = Some(writer);

        // Create a writer for the callback
        let writer_for_callback =
            Arc::new(Mutex::new(Some(WavWriter::create(&filename, spec).unwrap())));

        self.is_recording = true;

        let err_fn = |err| eprintln!("an error occurred on the audio stream: {}", err);

        let stream = device
            .build_input_stream(
                &config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if let Some(writer_guard) =
                        writer_for_callback.lock().unwrap().as_mut()
                    {
                        for &sample in data {
                            // Apply gain (increase volume) - adjust the multiplier as needed
                            let amplified_sample = sample * 2.5; // Increase gain

                            // Clamp to avoid distortion
                            let clamped_sample = amplified_sample.clamp(-1.0, 1.0);

                            // Convert f32 to i16 for the WAV file
                            let sample = (clamped_sample * 32767.0) as i16;
                            writer_guard.write_sample(sample).unwrap();
                        }
                    }
                },
                err_fn,
                None,
            )
            .expect("Failed to build input stream");

        stream.play().expect("Failed to start audio stream");
        self.stream = Some(stream);

        println!("Recording started to {}", filename);
    }

    fn stop_recording_and_save(&mut self) {
        if !self.is_recording {
            return;
        }

        self.is_recording = false;

        // Drop the stream to stop recording
        self.stream = None;

        // Finalize the writer if it exists
        if let Some(writer) = self.writer.take() {
            match writer.finalize() {
                Ok(_) => println!("Recording saved"),
                Err(e) => eprintln!("Error finalizing recording: {}", e),
            }
        }
    }
}

// Create a thread-local recorder
thread_local! {
    static RECORDER: std::cell::RefCell<AudioRecorder> = std::cell::RefCell::new(AudioRecorder::new());
}

// Create a command to toggle recording
#[tauri::command]
fn toggle_recording() {
    RECORDER.with(|recorder| {
        let mut recorder = recorder.borrow_mut();
        if recorder.is_recording {
            recorder.stop_recording_and_save();
        } else {
            recorder.start_recording();
        }
    });
}

#[tokio::main]
pub async fn main() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            app.listen("toggle-recording", |_| {
                toggle_recording();
            });

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .build(app)?;

            Ok(())
        })
        .on_tray_icon_event(|app_handle, event| {
            if let TrayIconEvent::Click { button_state, .. } = event {
                if button_state == MouseButtonState::Up {
                    app_handle.emit("toggle-recording", ()).unwrap();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![toggle_recording])
        .plugin(tauri_plugin_clipboard_manager::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

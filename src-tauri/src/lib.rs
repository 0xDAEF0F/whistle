use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{WavSpec, WavWriter};
use std::fs::File;
use std::io::BufWriter;
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Listener};

struct AudioRecorder {
    stream: Option<cpal::Stream>,
    writer: WavWriter<BufWriter<File>>,
    is_recording: bool,
}

impl AudioRecorder {
    fn new() -> Self {
        // Configure audio host and input device
        let host = cpal::default_host();
        let device = host.default_input_device().expect("No input device available");
        let config = device.default_input_config().unwrap();

        // Configure WAV file
        let spec = WavSpec {
            channels: config.channels() as u16,
            sample_rate: config.sample_rate().0,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        // Create WAV writer
        let writer = WavWriter::create("output.wav", spec).unwrap();

        Self {
            stream: None,
            writer,
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

        // Generate a timestamp for the filename
        // let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        // let filename = format!("recording_{}.wav", timestamp);
        let filename = "recording.wav".to_string();

        // Configure WAV file
        let spec = WavSpec {
            channels: config.channels() as u16,
            sample_rate: config.sample_rate().0,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        // Create a new writer
        self.writer = WavWriter::create(&filename, spec).unwrap();

        // Create a writer for the callback
        let writer_clone = std::sync::Arc::new(std::sync::Mutex::new(Some(
            WavWriter::create(&filename, spec).unwrap(),
        )));

        let writer_for_callback = writer_clone.clone();
        self.is_recording = true;

        let err_fn = |err| eprintln!("an error occurred on the audio stream: {}", err);

        let stream = device
            .build_input_stream(
                &config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if let Some(mut writer_guard) =
                        writer_for_callback.lock().unwrap().as_mut()
                    {
                        for &sample in data {
                            // Convert f32 to i16 for the WAV file
                            let sample = (sample * 32767.0) as i16;
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

        // Configure new writer for replacement
        let host = cpal::default_host();
        let device = host.default_input_device().expect("No input device available");
        let config = device.default_input_config().unwrap();

        let spec = WavSpec {
            channels: config.channels() as u16,
            sample_rate: config.sample_rate().0,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        // Take ownership of writer using replace
        let old_writer = std::mem::replace(
            &mut self.writer,
            WavWriter::create("output.wav", spec).unwrap(),
        );

        // Finalize the old writer
        match old_writer.finalize() {
            Ok(_) => println!("Recording saved"),
            Err(e) => eprintln!("Error finalizing recording: {}", e),
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
            if let TrayIconEvent::Click { .. } = event {
                app_handle.emit("toggle-recording", ()).unwrap();
            }
        })
        .invoke_handler(tauri::generate_handler![toggle_recording])
        .plugin(tauri_plugin_clipboard_manager::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

use anyhow::{Context, Result, bail};
use colored::Colorize;
use cpal::{
    Stream,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use hound::{WavSpec, WavWriter};
use std::sync::{Arc, Mutex};
use tempfile::NamedTempFile;

pub struct AudioRecorder {
    stream: Option<Stream>,
    sample_rate: Option<u32>,
    channels: Option<u16>,
    samples: Arc<Mutex<Vec<i16>>>,
    pub is_recording: bool,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            stream: None,
            sample_rate: None,
            channels: None,
            samples: Arc::new(Mutex::new(Vec::new())),
            is_recording: false,
        }
    }

    pub fn reset(&mut self) {
        self.stream = None;
        self.sample_rate = None;
        self.channels = None;
        self.samples.lock().unwrap().clear();
        self.is_recording = false;
    }

    pub fn start_recording(&mut self) -> Result<()> {
        if self.is_recording {
            bail!("'AudioRecorder' is already recording, skipping...");
        }

        let device = cpal::default_host()
            .default_input_device()
            .context("No input device available")?;
        let config = device.default_input_config()?;

        // Store audio format information
        self.sample_rate = Some(config.sample_rate().0);
        self.channels = Some(config.channels());

        // Clear previous samples
        self.samples.lock().unwrap().clear();

        // Create a samples buffer for the callback
        let samples_for_callback = self.samples.clone();

        self.is_recording = true;
        log::debug!(
            "'AudioRecorder' is recording: {} (should be true)",
            self.is_recording
        );

        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
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
        )?;

        stream.play()?;

        self.stream = Some(stream);

        Ok(())
    }

    pub fn stop_recording_and_get_bytes(&mut self) -> Option<Vec<u8>> {
        if !self.is_recording {
            return None;
        }

        log::debug!(
            "'AudioRecorder' switching state from {} to {}",
            self.is_recording,
            false
        );
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

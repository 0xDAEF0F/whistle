use crate::{
    audio_recorder::AudioRecorder,
    enigo_instance::EnigoInstance,
    notifications::{AppNotifications, Notification},
    transcribe_icon::{Icon, TranscribeIcon},
};
use anyhow::Result;
use std::{cell::RefCell, rc::Rc};
use tauri::{AppHandle, Manager};
use tokio::{
    sync::{
        mpsc::{self, Sender},
        oneshot,
    },
    task::LocalSet,
};

/// Tasks that will only be run on a `LocalSet`
pub enum Task {
    ToggleRecording(oneshot::Sender<Vec<u8>>),
    PasteFromClipboard,
    UndoText(oneshot::Sender<()>),
    CancelRecording,
}

/// - Instantiates its own tokio runtime
pub fn run_local_task_handler(mut rx: mpsc::Receiver<Task>, app_handle: AppHandle) {
    log::info!("Starting `run_local_task_handler`");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");
    let local = LocalSet::new();

    local.spawn_local(async move {
        let enigo = EnigoInstance::try_new();
        if enigo.is_err() {
            log::error!("Failed to create EnigoInstance");
            AppNotifications::new(&app_handle).notify(Notification::AccessibilityError);
            app_handle.exit(1);
        }
        let enigo = Rc::new(RefCell::new(enigo.unwrap()));
        let audio_recorder = Rc::new(RefCell::new(AudioRecorder::new()));
        let media_manager = Rc::new(RefCell::new(MediaManager::new()));
        while let Some(task) = rx.recv().await {
            let enigo = Rc::clone(&enigo);
            let audio_recorder = Rc::clone(&audio_recorder);
            let media_manager = Rc::clone(&media_manager);
            let app_handle = app_handle.clone();
            tokio::task::spawn_local(async move {
                match task {
                    Task::ToggleRecording(tx_recording) => {
                        log::info!("ToggleRecording task received through channel");

                        let mut recorder = audio_recorder.borrow_mut();
                        let mut media_manager = media_manager.borrow_mut();

                        if !recorder.is_recording {
                            media_manager.pause_spotify();
                            if let Err(e) = recorder.start_recording(
                                app_handle.state::<Sender<Task>>().clone(),
                            ) {
                                log::error!("Failed to start recording: {}", e);
                                recorder.reset();
                                return;
                            }
                            _ = tx_recording.send(vec![]);
                            return;
                        }

                        let Some(recording_bytes) =
                            recorder.stop_recording_and_get_bytes()
                        else {
                            log::error!("Failed to stop recording");
                            return;
                        };
                        media_manager.play_spotify();

                        if tx_recording.send(recording_bytes).is_err() {
                            log::error!("Failed to send recording to channel");
                        }
                    }
                    Task::PasteFromClipboard => {
                        enigo.borrow_mut().paste_from_clipboard().unwrap();
                    }
                    Task::UndoText(tx_undo) => {
                        enigo.borrow_mut().undo_text().unwrap();
                        tx_undo.send(()).unwrap();
                    }
                    Task::CancelRecording => {
                        let mut recorder = audio_recorder.borrow_mut();
                        if recorder.stop_recording_and_get_bytes().is_none() {
                            log::error!("Failed to stop recording");
                            return;
                        }
                        let icon = app_handle.state::<TranscribeIcon>();
                        icon.change_icon(Icon::Default);
                        AppNotifications::new(&app_handle)
                            .notify(Notification::CancelledSilence);

                        log::info!("Recording cancelled and icon changed to default");
                    }
                }
            });
        }
    });

    log::info!("Blocking on local task handler");
    rt.block_on(local);
    log::info!("Local task handler completed");
}

struct MediaManager {
    was_playing: bool,
}

impl MediaManager {
    fn new() -> Self {
        Self { was_playing: false }
    }

    pub fn pause_spotify(&mut self) {
        if let Err(e) = self.pause_spotify_() {
            log::error!("Failed to pause Spotify: {}", e);
        }
    }

    fn pause_spotify_(&mut self) -> Result<()> {
        let output = std::process::Command::new("osascript")
        .args(["-e", "tell application \"System Events\" to (name of processes) contains \"Spotify\""])
        .output()?;
        let is_running = String::from_utf8(output.stdout)?.trim() == "true";

        if !is_running {
            return Ok(());
        }

        let output = std::process::Command::new("osascript")
            .args(["-e", "tell application \"Spotify\" to player state"])
            .output()?;
        let is_playing = String::from_utf8(output.stdout)?.trim() == "playing";

        if is_running && is_playing {
            std::process::Command::new("osascript")
                .args(["-e", "tell application \"Spotify\" to pause"])
                .output()?;
            self.was_playing = true;
        }

        Ok(())
    }

    pub fn play_spotify(&mut self) {
        if let Err(e) = self.play_spotify_() {
            log::error!("Failed to play Spotify: {}", e);
        }
    }

    fn play_spotify_(&mut self) -> Result<()> {
        if !self.was_playing {
            return Ok(());
        }

        std::process::Command::new("osascript")
            .args(["-e", "tell application \"Spotify\" to play"])
            .output()?;

        self.was_playing = false;

        Ok(())
    }
}

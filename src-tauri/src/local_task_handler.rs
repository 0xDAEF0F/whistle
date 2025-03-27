use crate::{
    audio_recorder::AudioRecorder,
    media_manager::MediaManager,
    notifications::{AppNotifications, Notification},
    transcribe_icon::{Icon, TranscribeIcon},
};
use anyhow::Result;
use rdev::{EventType, Key, simulate};
use std::{cell::RefCell, rc::Rc, thread::sleep};
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

/// This should only be called on the main thread
fn paste_from_clipboard() -> Result<()> {
    simulate(&EventType::KeyPress(Key::MetaLeft))?;
    sleep(std::time::Duration::from_millis(20));
    simulate(&EventType::KeyPress(Key::KeyV))?;
    sleep(std::time::Duration::from_millis(20));
    simulate(&EventType::KeyRelease(Key::MetaLeft))?;
    sleep(std::time::Duration::from_millis(20));
    simulate(&EventType::KeyRelease(Key::KeyV))?;
    Ok(())
}

/// Instantiates its own tokio runtime
pub fn run_local_task_handler(mut rx: mpsc::Receiver<Task>, app_handle: AppHandle) {
    log::info!("Starting `run_local_task_handler`");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");
    let local = LocalSet::new();

    local.spawn_local(async move {
        let audio_recorder = Rc::new(RefCell::new(AudioRecorder::new()));
        let media_manager = Rc::new(RefCell::new(MediaManager::new()));
        while let Some(task) = rx.recv().await {
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
                    Task::PasteFromClipboard => match paste_from_clipboard() {
                        Ok(()) => log::info!("Pasted from clipboard successfully"),
                        Err(e) => log::error!("Failed to paste from clipboard: {}", e),
                    },
                    Task::UndoText(_tx_undo) => {
                        unimplemented!(
                            "UndoText task received through channel (not implemented)"
                        );
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

    rt.block_on(local);
}

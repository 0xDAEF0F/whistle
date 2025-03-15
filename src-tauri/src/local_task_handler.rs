use crate::{
    audio_recorder::AudioRecorder,
    enigo_instance::EnigoInstance,
    transcribe_icon::{Icon, TranscribeIcon},
};
use anyhow::Result;
use std::{cell::RefCell, rc::Rc};
use tauri::{AppHandle, Manager};
use tokio::{
    process,
    sync::{mpsc, oneshot},
    task::LocalSet,
};

/// Tasks that will only be run on a `LocalSet`
pub enum Task {
    ToggleRecording(oneshot::Sender<Vec<u8>>, AppHandle),
    PasteFromClipboard,
    UndoText(oneshot::Sender<()>),
}

/// - Instantiates its own tokio runtime
pub fn run_local_task_handler(mut rx: mpsc::Receiver<Task>) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");
    let local = LocalSet::new();
    local.spawn_local(async move {
        let enigo = Rc::new(RefCell::new(
            EnigoInstance::try_new().expect("Failed to create EnigoInstance"),
        ));
        let audio_recorder = Rc::new(RefCell::new(AudioRecorder::new()));
        while let Some(task) = rx.recv().await {
            let enigo = Rc::clone(&enigo);
            let audio_recorder = Rc::clone(&audio_recorder);
            tokio::task::spawn_local(async move {
                match task {
                    Task::ToggleRecording(tx_recording, app_handle) => {
                        pause_spotify().await.unwrap();

                        let mut recorder = audio_recorder.borrow_mut();
                        let transcribe_icon = app_handle.state::<TranscribeIcon>();

                        if !recorder.is_recording {
                            transcribe_icon.change_icon(Icon::Recording).unwrap();
                            recorder.start_recording();
                            _ = tx_recording.send(vec![]);
                            return;
                        }

                        transcribe_icon.change_icon(Icon::Default).unwrap();
                        let Some(recording_bytes) =
                            recorder.stop_recording_and_get_bytes()
                        else {
                            log::error!("Failed to stop recording");
                            return;
                        };

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
                }
            });
        }
    });
    rt.block_on(local);
}

async fn pause_spotify() -> Result<()> {
    let output = process::Command::new("osascript")
        .args(["-e", "tell application \"System Events\" to (name of processes) contains \"Spotify\""])
        .output()
        .await?;

    let is_running = String::from_utf8(output.stdout)?.trim() == "true";

    if is_running {
        process::Command::new("osascript")
            .args(["-e", "tell application \"Spotify\" to pause"])
            .output()
            .await?;
    }

    Ok(())
}

use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub enum Notification {
    StartPolishing,
    PolishSuccess,
    TranscribeSuccess, // when not pasting from clipboard
    ApiError,
    AccessibilityError,
    CancelledSilence,
}

pub struct AppNotifications<'a> {
    app_handle: &'a AppHandle,
}

impl<'a> AppNotifications<'a> {
    pub fn new(app_handle: &'a AppHandle) -> Self {
        Self { app_handle }
    }

    pub fn notify(&self, notification: Notification) {
        let notifs = self.app_handle.notification().builder();
        if let Err(e) = match notification {
            Notification::PolishSuccess => notifs
                .title("Polishing complete")
                .body("Your polished text is ready and in your clipboard")
                .show(),
            Notification::StartPolishing => notifs
                .title("Loading...")
                .body("We're starting to polish your text. Please wait")
                .show(),
            Notification::TranscribeSuccess => notifs
                .title("Transcription complete")
                .body("Your transcription is ready in your clipboard")
                .show(),
            Notification::ApiError => notifs
                .title("Error")
                .body("Failed to connect to the API. Please try again later")
                .show(),
            Notification::AccessibilityError => notifs
                .title("Error")
                .body("Please grant accessibility permissions to the app and restart it")
                .show(),
            Notification::CancelledSilence => notifs
                .title("Recording cancelled")
                .body("No sound detected for a while, recording cancelled")
                .show(),
        } {
            log::error!("Failed to trigger notification: {}", e);
        }
    }
}

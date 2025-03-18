use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub enum Notification {
    StartPolishing,
    PolishSuccess,
    TranscribeSuccess, // when not pasting from clipboard
    ApiError,
    AccessibilityError,
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
                .title("Done")
                .body("Your polished text is ready and in your clipboard")
                .show(),
            Notification::StartPolishing => notifs
                .title("Loading...")
                .body("We're starting to polish your text. Please wait")
                .show(),
            Notification::TranscribeSuccess => notifs
                .title("Done")
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
        } {
            log::error!("Failed to trigger notification: {}", e);
        }
    }
}

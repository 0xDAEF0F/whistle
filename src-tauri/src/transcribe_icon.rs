use anyhow::Result;
use tauri::{image::Image, tray::TrayIcon};

#[derive(Debug, Clone, Copy)]
pub enum Icon {
    Default,
    Recording,
    Transcribing,
    Cleansing,
}

pub struct TranscribeIcon(TrayIcon);

impl TranscribeIcon {
    pub fn new(tray_icon: TrayIcon) -> Self {
        Self(tray_icon)
    }

    pub fn change_icon(&self, icon: Icon) -> Result<()> {
        let img = match icon {
            Icon::Default => Image::from_path("icons/StoreLogo.png")?, // TODO
            Icon::Recording => Image::from_path("icons/recording-icon.png")?,
            Icon::Transcribing => Image::from_path("icons/transcribing-icon.png")?,
            Icon::Cleansing => Image::from_path("icons/transcribing-icon.png")?, // TODO
        };

        self.0.set_icon(Some(img))?;

        Ok(())
    }
}

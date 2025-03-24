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

    pub fn change_icon(&self, icon: Icon) {
        if let Err(e) = self.change_icon_(icon) {
            log::error!("Unable to change icon: {e}");
        } else {
            log::trace!("Successfully changed icon to: {icon:?}");
        }
    }

    fn change_icon_(&self, icon: Icon) -> Result<()> {
        let img = match icon {
            Icon::Default => Image::from_bytes(include_bytes!("../icons/whistle.png"))?,
            Icon::Recording => {
                Image::from_bytes(include_bytes!("../icons/recording-icon.png"))?
            }
            Icon::Transcribing => {
                Image::from_bytes(include_bytes!("../icons/transcribing-icon.png"))?
            }
            Icon::Cleansing => {
                Image::from_bytes(include_bytes!("../icons/transcribing-icon.png"))?
            }
        };

        self.0.set_icon(Some(img))?;

        Ok(())
    }
}

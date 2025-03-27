use anyhow::Result;

pub struct MediaManager {
    was_playing: bool,
}

impl MediaManager {
    pub fn new() -> Self {
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

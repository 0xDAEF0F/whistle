use anyhow::Result;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use tokio::sync::mpsc::Receiver;

pub enum EnigoCommand {
    PasteFromClipboard,
    UndoText,
}

pub struct EnigoInstance {
    enigo: Enigo,
}

impl EnigoInstance {
    pub fn try_new() -> Result<Self> {
        let settings = Settings::default();
        let enigo = Enigo::new(&settings)?;
        Ok(Self { enigo })
    }

    pub fn paste_from_clipboard(&mut self) -> Result<()> {
        self.enigo.key(Key::Meta, Direction::Press)?;
        self.enigo.key(Key::Unicode('v'), Direction::Click)?;
        self.enigo.key(Key::Meta, Direction::Release)?;
        Ok(())
    }

    pub fn undo_text(&mut self) -> Result<()> {
        self.enigo.key(Key::Meta, Direction::Press)?;
        self.enigo.key(Key::Unicode('z'), Direction::Click)?;
        self.enigo.key(Key::Meta, Direction::Release)?;
        Ok(())
    }
}

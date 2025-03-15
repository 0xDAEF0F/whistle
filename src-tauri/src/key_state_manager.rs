use device_query::Keycode;
use std::collections::HashSet;

pub enum TranscribeAction {
    TranscribeEnglish,
    CleanseTranscription, // from clipboard
}

pub struct KeyStateManager(HashSet<Keycode>);

impl KeyStateManager {
    pub fn new() -> Self {
        Self(HashSet::new())
    }

    pub fn add_key(&mut self, key: Keycode) {
        self.0.insert(key);
    }

    pub fn remove_key(&mut self, key: &Keycode) {
        self.0.remove(key);
    }

    pub fn match_action(&self) -> Option<TranscribeAction> {
        use Keycode::*;
        if self.0.is_superset(&[F19].into()) {
            return Some(TranscribeAction::TranscribeEnglish);
        }
        if self.0.is_superset(&[F20].into()) {
            return Some(TranscribeAction::CleanseTranscription);
        }
        None
    }

    pub fn keys_in_question() -> [Keycode; 2] {
        use Keycode::*;
        [F19, F20]
    }
}

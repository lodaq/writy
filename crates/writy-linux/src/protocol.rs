use serde::{Deserialize, Serialize};
use writy_core::types::{LexicalEntry, Suggestion, TextSnapshot};

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    Search,
    Settings,
    Lookup { text: String },
    Accept,
    Copy,
    Pause,
    Prose,
    Dismiss,
    Status,
    HudInteraction { active: bool },
    Offline { enabled: bool },
    Query { text: String, translate: bool },
    Reload,
    Selection { text: Option<String> },
}
impl Command {
    pub fn validate(&self) -> Result<(), &'static str> {
        let input = match self {
            Self::Lookup { text }
            | Self::Query { text, .. }
            | Self::Selection { text: Some(text) } => Some(text),
            _ => None,
        };
        if input.is_some_and(|text| text.chars().count() > 8192) {
            return Err("Input exceeds 8192 characters; shorten it before translating");
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeEvent {
    Hello {
        version: u32,
        seq: u64,
    },
    Focus {
        app_id: String,
        window_id: String,
        focus_epoch: u64,
        seq: u64,
        #[serde(default)]
        pid: u32,
        #[serde(default)]
        locked: bool,
        #[serde(default)]
        monitor: String,
        #[serde(default)]
        monitor_geometry: Option<(i32, i32, i32, i32)>,
    },
    Keymap {
        device: String,
        keymap: String,
        seq: u64,
    },
    Modifiers {
        device: String,
        depressed: u32,
        latched: u32,
        locked: u32,
        group: u32,
        seq: u64,
    },
    Key {
        device: String,
        keycode: u32,
        state: u32,
        focus_epoch: u64,
        seq: u64,
    },
    Reset {
        #[serde(default)]
        reason: String,
        seq: u64,
        #[serde(default)]
        focus_epoch: u64,
    },
}
impl BridgeEvent {
    pub fn seq(&self) -> u64 {
        match self {
            Self::Hello { seq, .. }
            | Self::Focus { seq, .. }
            | Self::Keymap { seq, .. }
            | Self::Modifiers { seq, .. }
            | Self::Key { seq, .. }
            | Self::Reset { seq, .. } => *seq,
        }
    }
}
#[derive(Clone)]
pub struct Results {
    pub writing: bool,
    pub output_bounds: Option<(i32, i32, i32, i32)>,
    pub generation: u64,
    pub snapshot: TextSnapshot,
    pub entries: Vec<LexicalEntry>,
    pub suggestions: Vec<Suggestion>,
    pub translation: Option<(String, String)>,
    pub status: String,
    pub caret: Option<(i32, i32, i32, i32)>,
    pub output: Option<String>,
}
pub enum UiEvent {
    Clear {
        generation: u64,
        preserve_huds: bool,
    },
    Screenshot(bool),
    Dismiss,
    Search(Option<String>),
    Settings,
    Results(Box<Results>),
    Notice(String),
    Copy(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn query_limits_count_unicode_characters_and_reject_instead_of_truncating() {
        assert!(
            Command::Lookup {
                text: "م".repeat(8192)
            }
            .validate()
            .is_ok()
        );
        assert!(
            Command::Lookup {
                text: "م".repeat(8193)
            }
            .validate()
            .is_err()
        );
        assert!(
            Command::Query {
                text: "x".repeat(8193),
                translate: true
            }
            .validate()
            .is_err()
        );
        assert!(
            Command::Selection {
                text: Some("x".repeat(8193))
            }
            .validate()
            .is_err()
        );
    }
}

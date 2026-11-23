//! Window state remembered between launches.
use crate::config;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

/// The last activated tab in each window.
///
/// This lives in its own file so switching tabs cannot rewrite model keys or
/// dictionary preferences. [`UiState::load`] adds the disk location and writes
/// changes back; a [`Default`] value keeps them in memory for tests.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UiState {
    /// Category key of the selected search result tab, or `sources`.
    pub search_tab: String,
    /// Category key of the selected floating panel section.
    pub hud_tab: String,
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl UiState {
    pub fn load() -> Self {
        Self::load_at(&Self::path())
    }
    /// Missing or malformed state starts from defaults; a preference must never
    /// prevent the application from opening.
    pub fn load_at(path: &Path) -> Self {
        let mut state: Self = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default();
        state.path = Some(path.to_path_buf());
        state
    }
    pub fn save(&self) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let dir = path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir)?;
        let temp = dir.join(format!(".ui-{}.tmp", std::process::id()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| -> Result<()> {
            let mut file = options.open(&temp)?;
            file.write_all(toml::to_string_pretty(self)?.as_bytes())?;
            file.sync_all()?;
            std::fs::rename(&temp, path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result
    }
    pub fn path() -> PathBuf {
        state_dir().join("ui.toml")
    }
}

pub fn state_dir() -> PathBuf {
    config::xdg_dir("XDG_STATE_HOME", ".local/state").join("writy")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("writy-state-{}-{name}.toml", std::process::id()))
    }

    #[test]
    fn tabs_round_trip_and_bad_state_falls_back() {
        let path = temp("round-trip");
        let _ = std::fs::remove_file(&path);
        let missing = UiState::load_at(&path);
        assert!(missing.search_tab.is_empty() && missing.hud_tab.is_empty());
        let mut state = UiState::load_at(&path);
        state.search_tab = "definition".into();
        state.hud_tab = "examples".into();
        state.save().unwrap();
        let restored = UiState::load_at(&path);
        assert_eq!(restored.search_tab, "definition");
        assert_eq!(restored.hud_tab, "examples");
        std::fs::write(&path, "not = [valid").unwrap();
        let broken = UiState::load_at(&path);
        assert!(broken.search_tab.is_empty() && broken.hud_tab.is_empty());
        let _ = std::fs::remove_file(&path);
    }
}

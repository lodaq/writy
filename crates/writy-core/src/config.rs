use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub offline: bool,
    pub selection_popup: bool,
    pub typing: Typing,
    pub model: Models,
    pub apps: Apps,
    pub huds: BTreeMap<String, bool>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Typing {
    pub idle_ms: u64,
    pub min_rephrase_words: usize,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Models {
    pub ollama_url: String,
    pub rewrite_model: String,
    pub translation_model: String,
    pub cloud_url: String,
    pub cloud_model: String,
    pub api_key_env: String,
    pub timeout_secs: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Apps {
    pub blocklist: Vec<String>,
    pub terminals: Vec<String>,
}
impl Default for Typing {
    fn default() -> Self {
        Self {
            idle_ms: 300,
            min_rephrase_words: 2,
        }
    }
}
impl Default for Models {
    fn default() -> Self {
        Self {
            ollama_url: "http://127.0.0.1:11434".into(),
            rewrite_model: "gemma3:4b".into(),
            translation_model: "translategemma:4b".into(),
            cloud_url: String::new(),
            cloud_model: String::new(),
            api_key_env: "WRITY_API_KEY".into(),
            timeout_secs: 30,
        }
    }
}
impl Default for Apps {
    fn default() -> Self {
        Self {
            blocklist: vec![
                "keepass".into(),
                "bitwarden".into(),
                "1password".into(),
                "org.gnome.screensaver".into(),
            ],
            terminals: vec![
                "foot".into(),
                "alacritty".into(),
                "kitty".into(),
                "wezterm".into(),
                "org.gnome.terminal".into(),
                "com.mitchellh.ghostty".into(),
            ],
        }
    }
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            offline: false,
            selection_popup: true,
            typing: Typing::default(),
            model: Models::default(),
            apps: Apps::default(),
            huds: [
                "translation",
                "definition",
                "examples",
                "collocations",
                "synonyms",
                "word_family",
                "suggestions",
            ]
            .into_iter()
            .map(|s| (s.into(), true))
            .collect(),
        }
    }
}
impl Config {
    pub fn path() -> PathBuf {
        config_dir().join("config.toml")
    }
    pub fn load() -> Result<Self> {
        let config: Self = match std::fs::read_to_string(Self::path()) {
            Ok(s) => toml::from_str(&s)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => return Err(e.into()),
        };
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (50..=10000).contains(&self.typing.idle_ms),
            "typing.idle_ms must be 50..10000"
        );
        ensure!(
            (1..=100).contains(&self.typing.min_rephrase_words),
            "typing.min_rephrase_words must be 1..100"
        );
        ensure!(
            (1..=120).contains(&self.model.timeout_secs),
            "model.timeout_secs must be 1..120"
        );
        let url = reqwest::Url::parse(&self.model.ollama_url)?;
        ensure!(
            matches!(
                url.host_str(),
                Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
            ),
            "Ollama must use a loopback endpoint"
        );
        if !self.model.cloud_url.is_empty() {
            ensure!(
                reqwest::Url::parse(&self.model.cloud_url)?.scheme() == "https",
                "cloud endpoint must use HTTPS"
            );
        }
        Ok(())
    }
    pub fn capture_allowed(&self, app: &str, prose: bool, secure: bool) -> bool {
        let app = app.to_lowercase();
        self.enabled
            && !secure
            && !app.is_empty()
            && !app.contains("writy")
            && !self
                .apps
                .blocklist
                .iter()
                .any(|b| app.contains(&b.to_lowercase()))
            && (prose
                || !self
                    .apps
                    .terminals
                    .iter()
                    .any(|t| app.contains(&t.to_lowercase())))
    }
    pub fn save(&self) -> Result<()> {
        use std::io::Write;
        self.validate()?;
        let dir = config_dir();
        std::fs::create_dir_all(&dir)?;
        let temp = dir.join(format!(".config-{}.tmp", std::process::id()));
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
            std::fs::rename(&temp, Self::path())?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result
    }
}
pub fn config_dir() -> PathBuf {
    xdg_dir("XDG_CONFIG_HOME", ".config").join("writy")
}
pub fn data_dir() -> PathBuf {
    xdg_dir("XDG_DATA_HOME", ".local/share").join("writy")
}
fn xdg_dir(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(fallback)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_and_blocked_policies() {
        let c = Config::default();
        assert!(!c.capture_allowed("foot", false, false));
        assert!(c.capture_allowed("foot", true, false));
        assert!(!c.capture_allowed("Bitwarden", true, false));
        assert!(!c.capture_allowed("editor", false, true));
    }
    #[test]
    fn defaults_roundtrip_and_validate() {
        let c: Config =
            toml::from_str(&toml::to_string_pretty(&Config::default()).unwrap()).unwrap();
        c.validate().unwrap();
    }
}

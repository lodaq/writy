mod accessibility;
mod backend;
mod input;
mod ipc;
mod protocol;
mod ui;

use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use protocol::Command;
use std::{io::Read, path::PathBuf};
use writy_core::{
    config::{Config, data_dir},
    dictionary::{Dictionary, OpenPack, download_open_pack},
    types::{Attribution, Language},
};

#[derive(Parser)]
#[command(
    name = "writy",
    version,
    about = "English ↔ Arabic writing and reading assistance for Hyprland"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Action>,
}
#[derive(Subcommand)]
enum Action {
    /// Run the background service and native HUDs.
    Daemon,
    /// Open the focused bilingual search/editor window.
    Search,
    /// Alias for search.
    Translate,
    /// Look up text or the current PRIMARY selection.
    Lookup {
        #[arg(num_args = 0..)]
        text: Vec<String>,
        #[arg(long)]
        selection: bool,
        #[arg(long)]
        json: bool,
    },
    Accept,
    Copy,
    Pause,
    Prose,
    Dismiss,
    Status,
    Settings,
    /// List all model tags installed in the configured Ollama service.
    Models,
    Offline {
        #[arg(long)]
        disable: bool,
    },
    /// Manage local attributed dictionaries.
    Dict {
        #[command(subcommand)]
        action: DictAction,
    },
    /// Print an example configuration without modifying your settings.
    Config,
    /// Run local English checks from stdin or arguments.
    Check {
        text: Vec<String>,
    },
    /// Show installed platform dependencies without reading application text.
    Doctor,
    #[command(hide = true)]
    SelectionEvent,
}
#[derive(Subcommand)]
enum DictAction {
    Install {
        #[arg(value_parser=["en-ar","ar-en","both"],default_value="both")]
        pack: String,
    },
    Import {
        path: PathBuf,
        #[arg(long,value_parser=["en","ar"],default_value="en")]
        language: String,
        #[arg(long)]
        name: Option<String>,
        /// Treat StarDict payloads as bilingual translations, not definitions.
        #[arg(long)]
        bilingual: bool,
        #[arg(long, default_value = "User-supplied; verify source terms")]
        license: String,
        #[arg(long, default_value = "")]
        url: String,
    },
    List,
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Writy: {e:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
fn runtime() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?)
}
fn run() -> Result<()> {
    let action = Cli::parse().command.unwrap_or(Action::Search);
    if matches!(action, Action::Daemon) {
        let config = Config::load()?;
        let rt = runtime()?;
        if rt.block_on(ipc::send(&Command::Status)).is_ok() {
            anyhow::bail!("daemon is already running");
        }
        let exit = ui::run(config);
        ensure!(
            exit == gtk::glib::ExitCode::SUCCESS,
            "desktop application failed"
        );
        return Ok(());
    }
    if matches!(action, Action::Config) {
        println!("{}", toml::to_string_pretty(&Config::default())?);
        return Ok(());
    }
    if let Action::Check { text } = action {
        let text = if text.is_empty() {
            read_stdin()?
        } else {
            text.join(" ")
        };
        let mut buffer = writy_core::buffer::TypingBuffer::default();
        buffer.push(&text);
        let suggestions = writy_core::linter::EnglishChecker::default().check(&buffer.snapshot());
        println!("{}", serde_json::to_string_pretty(&suggestions)?);
        return Ok(());
    }
    if matches!(action, Action::Models) {
        let config = Config::load()?;
        let models = runtime()?.block_on(writy_core::provider::ollama_models(
            &config.model.ollama_url,
        ))?;
        for model in models {
            println!("{model}");
        }
        return Ok(());
    }
    if matches!(action, Action::Doctor) {
        for program in ["hyprctl", "wl-paste", "mpv", "ollama"] {
            let found = std::env::var_os("PATH").is_some_and(|path| {
                std::env::split_paths(&path).any(|p| p.join(program).is_file())
            });
            println!("{program}: {}", if found { "installed" } else { "missing" });
        }
        println!("Configuration: {}", Config::path().display());
        println!("Dictionary database: {}", ipc::database_path().display());
        return Ok(());
    }
    let rt = runtime()?;
    if let Action::Dict { action } = action {
        return rt.block_on(dictionary_action(action));
    }
    let command = match action {
        Action::Search | Action::Translate => Command::Search,
        Action::Settings => Command::Settings,
        Action::Accept => Command::Accept,
        Action::Copy => Command::Copy,
        Action::Pause => Command::Pause,
        Action::Prose => Command::Prose,
        Action::Dismiss => Command::Dismiss,
        Action::Status => Command::Status,
        Action::Offline { disable } => Command::Offline { enabled: !disable },
        Action::Lookup {
            text,
            selection,
            json,
        } => {
            let text = if selection {
                rt.block_on(primary_selection())?
            } else {
                text.join(" ")
            };
            ensure!(!text.trim().is_empty(), "provide text or --selection");
            if json {
                let dictionary = Dictionary::open(ipc::database_path())?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&dictionary.lookup(&text)?)?
                );
                return Ok(());
            }
            Command::Lookup { text }
        }
        Action::SelectionEvent => {
            let state = std::env::var("CLIPBOARD_STATE").unwrap_or_default();
            let text = if matches!(state.as_str(), "nil" | "clear" | "sensitive") {
                None
            } else {
                let s = read_stdin()?;
                if s.trim().is_empty() { None } else { Some(s) }
            };
            Command::Selection { text }
        }
        _ => unreachable!(),
    };
    let result = rt.block_on(ipc::send(&command));
    let answer = match result {
        Ok(answer) => answer,
        Err(_)
            if matches!(
                command,
                Command::Search | Command::Settings | Command::Lookup { .. }
            ) =>
        {
            std::process::Command::new(std::env::current_exe()?)
                .arg("daemon")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .spawn()?;
            rt.block_on(async {
                for _ in 0..40 {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    if let Ok(answer) = ipc::send(&command).await {
                        return Ok(answer);
                    }
                }
                anyhow::bail!("daemon did not start; run `writy daemon` for diagnostics")
            })?
        }
        Err(e) => return Err(e),
    };
    if !answer.trim().is_empty() {
        println!("{}", answer.trim());
    }
    Ok(())
}
fn read_stdin() -> Result<String> {
    let mut s = String::new();
    std::io::stdin().take(32769).read_to_string(&mut s)?;
    ensure!(s.len() <= 32768, "input exceeds limit");
    Ok(s)
}
async fn dictionary_action(action: DictAction) -> Result<()> {
    let path = ipc::database_path();
    ipc::ensure_parent(&path)?;
    match action {
        DictAction::List => {
            for (source, count) in Dictionary::open(path)?.sources()? {
                println!(
                    "{}\t{} entries\t{}\t{}",
                    source.name, count, source.license, source.url
                );
            }
        }
        DictAction::Import {
            path: source_path,
            language,
            name,
            bilingual,
            license,
            url,
        } => {
            let language = if language == "ar" {
                Language::Arabic
            } else {
                Language::English
            };
            let source = Attribution {
                name: name.unwrap_or_else(|| {
                    source_path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                }),
                url,
                license,
            };
            let mut dictionary = Dictionary::open(path)?;
            let count = if source_path.extension().is_some_and(|e| e == "ifo") {
                if bilingual {
                    dictionary.import_bilingual_stardict(&source_path, language, source)?
                } else {
                    dictionary.import_stardict(&source_path, language, source)?
                }
            } else {
                ensure!(
                    !bilingual,
                    "--bilingual applies to StarDict; Kaikki carries structured translations"
                );
                dictionary.import_kaikki(&source_path, language, source)?
            };
            println!("Imported {count} entries");
        }
        DictAction::Install { pack } => {
            let packs = match pack.as_str() {
                "en-ar" => vec![OpenPack::EnglishArabic],
                "ar-en" => vec![OpenPack::ArabicEnglish],
                _ => vec![OpenPack::EnglishArabic, OpenPack::ArabicEnglish],
            };
            for pack in packs {
                let ifo = download_open_pack(pack, &data_dir().join("packs")).await?;
                let count = Dictionary::open(&path)?.import_bilingual_stardict(
                    &ifo,
                    pack.language(),
                    pack.source(),
                )?;
                println!("Installed {}: {count} entries", pack.id());
            }
        }
    }
    let _ = ipc::send(&Command::Reload).await;
    Ok(())
}

async fn primary_selection() -> Result<String> {
    use tokio::io::AsyncReadExt;
    let mut child = tokio::process::Command::new("wl-paste")
        .args(["--primary", "--no-newline"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .context("wl-paste is required")?;
    let mut stdout = child
        .stdout
        .take()
        .context("selection reader unavailable")?
        .take(32769);
    let mut bytes = Vec::new();
    let read = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        stdout.read_to_end(&mut bytes),
    )
    .await;
    if !matches!(read, Ok(Ok(_))) || bytes.len() > 32768 {
        let _ = child.kill().await;
        anyhow::bail!("selection timed out or exceeded the 32768-byte limit");
    }
    let status = tokio::time::timeout(std::time::Duration::from_secs(1), child.wait())
        .await
        .context("selection reader timed out")??;
    ensure!(status.success(), "PRIMARY selection is unavailable");
    Ok(String::from_utf8(bytes)?)
}

#[cfg(test)]
mod cli_tests {
    use super::*;
    #[test]
    fn lookup_flags_are_not_swallowed_as_query_text() {
        let cli = Cli::try_parse_from(["writy", "lookup", "decision", "--json"]).unwrap();
        assert!(
            matches!(cli.command,Some(Action::Lookup{text,json:true,selection:false}) if text==["decision"])
        );
        let cli = Cli::try_parse_from(["writy", "lookup", "--selection"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Action::Lookup {
                selection: true,
                ..
            })
        ));
    }
}

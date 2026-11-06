//! Discoverable install sources and live, persistent per-source preferences.
use super::*;
use writy_core::dictionary::{Dictionary, DictionarySource};

struct CatalogueEntry {
    name: &'static str,
    prefix: &'static str,
    url: &'static str,
    detail: &'static str,
    pack: Option<&'static str>,
}
const CATALOGUE: &[CatalogueEntry] = &[
    CatalogueEntry {
        name: "FreeDict English → Arabic",
        prefix: "FreeDict eng-ara",
        url: "https://freedict.org/downloads/",
        detail: "Bilingual translations · GPL-2.0-or-later",
        pack: Some("en-ar"),
    },
    CatalogueEntry {
        name: "FreeDict Arabic → English",
        prefix: "FreeDict ara-eng",
        url: "https://freedict.org/downloads/",
        detail: "Bilingual translations · GPL-2.0-or-later",
        pack: Some("ar-en"),
    },
    CatalogueEntry {
        name: "Wiktionary English (Kaikki)",
        prefix: "Wiktionary English",
        url: "https://kaikki.org/dictionary/English/",
        detail: "Definitions, examples and word forms · Import plain JSONL · CC-BY-SA-4.0 / GFDL",
        pack: None,
    },
    CatalogueEntry {
        name: "Wiktionary Arabic (Kaikki)",
        prefix: "Wiktionary Arabic",
        url: "https://kaikki.org/dictionary/Arabic/",
        detail: "Arabic headwords with English glosses · Import plain JSONL · CC-BY-SA-4.0 / GFDL",
        pack: None,
    },
    CatalogueEntry {
        name: "Open English WordNet",
        prefix: "Open English WordNet",
        url: "https://github.com/globalwordnet/english-wordnet",
        detail: "Senses and semantic relations · Convert using tools/wordnet_to_jsonl.py, then import · CC-BY-4.0",
        pack: None,
    },
    CatalogueEntry {
        name: "Arramooz Alwaseet",
        prefix: "Arramooz",
        url: "https://sourceforge.net/projects/arramooz/files/",
        detail: "Arabic roots and morphology · Import a StarDict edition with its supplied GPL terms",
        pack: None,
    },
];

pub(super) fn populate(list: &gtk::Box, tx: &Commands, message: &gtk::Label) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let loading = label("Loading dictionary status…", "writy-status");
    list.append(&loading);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = Dictionary::open(crate::ipc::database_path())
            .and_then(|d| d.source_states())
            .map_err(|e| e.to_string());
        let _ = sender.send(result);
    });
    let list = list.downgrade();
    let tx = tx.clone();
    let message = message.clone();
    glib::timeout_add_local(Duration::from_millis(50), move || {
        let Some(list) = list.upgrade() else {
            return glib::ControlFlow::Break;
        };
        // A later refresh replaces the loading widget; discard older responses.
        if list.first_child().as_ref() != Some(loading.upcast_ref()) {
            return glib::ControlFlow::Break;
        }
        match receiver.try_recv() {
            Ok(Ok(sources)) => render(&list, &tx, &message, &sources),
            Ok(Err(error)) => loading.set_text(&format!("Dictionary status unavailable: {error}")),
            Err(std::sync::mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
            Err(_) => {
                loading.set_text("Dictionary discovery stopped. Try Refresh dictionary status.")
            }
        }
        glib::ControlFlow::Break
    });
}

pub(super) fn render(
    list: &gtk::Box,
    tx: &Commands,
    message: &gtk::Label,
    sources: &[DictionarySource],
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    for entry in CATALOGUE {
        let installed: Vec<_> = sources
            .iter()
            .filter(|s| s.attribution.name.starts_with(entry.prefix))
            .collect();
        if installed.is_empty() {
            let row = row(entry.name, entry.detail);
            row.append(&label("Not installed", "writy-dictionary-state"));
            let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            if let Some(pack) = entry.pack {
                let install = gtk::Button::with_label("Install");
                let pack = pack.to_owned();
                let list = list.clone();
                let tx = tx.clone();
                let message = message.clone();
                install.connect_clicked(move |button| {
                    install_pack(button, &pack, &list, &tx, &message)
                });
                actions.append(&install);
            }
            actions.append(&gtk::LinkButton::with_label(entry.url, "Download source"));
            row.append(&actions);
            list.append(&row);
        } else {
            for source in installed {
                installed_row(list, source, tx, message);
            }
        }
    }
    // Imported third-party and custom dictionaries are never hidden by the catalogue.
    for source in sources {
        if !CATALOGUE
            .iter()
            .any(|entry| source.attribution.name.starts_with(entry.prefix))
        {
            installed_row(list, source, tx, message);
        }
    }
    list.append(&label("Other dictionaries can be imported as StarDict (.ifo) or Kaikki JSONL with their attribution and licence. Downloading a source does not install it; import it with writy dict import.", "writy-meta"));
}
fn row(name: &str, detail: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 6);
    row.add_css_class("writy-dictionary-row");
    row.append(&label(name, "writy-title"));
    row.append(&label(detail, "writy-meta"));
    row
}
fn installed_row(list: &gtk::Box, source: &DictionarySource, tx: &Commands, message: &gtk::Label) {
    let row = row(&source.attribution.name, &source.attribution.license);
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let status = label(
        &installed_status(source.entries, source.enabled),
        "writy-dictionary-state",
    );
    status.set_hexpand(true);
    controls.append(&status);
    let toggle = gtk::Switch::new();
    toggle.set_valign(gtk::Align::Center);
    toggle.set_active(source.enabled);
    toggle.update_property(&[gtk::accessible::Property::Label(&format!(
        "Enable {}",
        source.attribution.name
    ))]);
    let id = source.id.clone();
    let count = source.entries;
    let tx = tx.clone();
    let message = message.clone();
    toggle.connect_state_set(move |_, enabled| {
        match Dictionary::open(crate::ipc::database_path())
            .and_then(|d| d.set_source_enabled(&id, enabled))
        {
            Ok(()) => {
                status.set_text(&installed_status(count, enabled));
                send(&tx, Command::Reload);
                message.set_text("Dictionary preference saved.");
                glib::Propagation::Proceed
            }
            Err(error) => {
                message.set_text(&format!("Dictionary preference not saved: {error}"));
                glib::Propagation::Stop
            }
        }
    });
    controls.append(&toggle);
    row.append(&controls);
    list.append(&row);
}
fn installed_status(count: usize, enabled: bool) -> String {
    format!(
        "Installed · {} · {count} entries",
        if enabled { "Enabled" } else { "Disabled" }
    )
}
fn install_pack(
    button: &gtk::Button,
    pack: &str,
    list: &gtk::Box,
    tx: &Commands,
    message: &gtk::Label,
) {
    let child = std::env::current_exe().and_then(|exe| {
        std::process::Command::new(exe)
            .args(["dict", "install", pack])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
    });
    let mut child = match child {
        Ok(child) => child,
        Err(error) => {
            message.set_text(&format!("Cannot start installation: {error}"));
            return;
        }
    };
    button.set_sensitive(false);
    message.set_text("Downloading and verifying dictionary…");
    let button = button.clone();
    let list = list.clone();
    let tx = tx.clone();
    let message = message.clone();
    glib::timeout_add_local(Duration::from_millis(250), move || {
        match child.try_wait() {
            Ok(None) => return glib::ControlFlow::Continue,
            Ok(Some(status)) if status.success() => {
                message.set_text("Dictionary installed and ready.");
                populate(&list, &tx, &message);
                send(&tx, Command::Reload);
            }
            _ => {
                button.set_sensitive(true);
                message.set_text("Installation failed. Try again or check writy dict install.");
            }
        }
        glib::ControlFlow::Break
    });
}

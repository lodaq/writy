//! Discoverable install sources and live, persistent per-source preferences.
use super::*;
use writy_core::dictionary::{CatalogueEntry, DictionaryLibrary, DictionarySource};

pub(super) use writy_core::dictionary::CATALOGUE;

thread_local! {
    static OPERATIONS: RefCell<std::collections::BTreeMap<&'static str, &'static str>> = const { RefCell::new(std::collections::BTreeMap::new()) };
}

fn begin_operation(name: &'static str, status: &'static str) -> bool {
    OPERATIONS.with(|active| {
        if let std::collections::btree_map::Entry::Vacant(entry) = active.borrow_mut().entry(name) {
            entry.insert(status);
            true
        } else {
            false
        }
    })
}

pub(super) fn watch_installs(list: &gtk::Box, tx: &Commands, message: &gtk::Label) {
    let list = list.downgrade();
    let tx = tx.clone();
    let message = message.clone();
    let mut previous = OPERATIONS.with(|active| active.borrow().clone());
    glib::timeout_add_local(Duration::from_millis(500), move || {
        let Some(list) = list.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let current = OPERATIONS.with(|active| active.borrow().clone());
        if current != previous {
            populate(&list, &tx, &message);
            previous = current;
        }
        glib::ControlFlow::Continue
    });
}

pub(super) fn populate(list: &gtk::Box, tx: &Commands, message: &gtk::Label) {
    let loading = label("Loading dictionary status…", "writy-status");
    // Keep the current library allocated while discovery runs, preserving scrolling.
    loading.set_visible(list.first_child().is_none());
    list.append(&loading);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = crate::ipc::open_dictionary()
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
        // The last marker belongs to the newest refresh; discard older responses.
        if list.last_child().as_ref() != Some(loading.upcast_ref()) {
            if loading.parent().as_ref() == Some(list.upcast_ref()) {
                list.remove(&loading);
            }
            return glib::ControlFlow::Break;
        }
        match receiver.try_recv() {
            Ok(Ok(sources)) => render(&list, &tx, &message, &sources),
            Ok(Err(error)) => {
                loading.set_visible(true);
                loading.set_text(&format!("Dictionary status unavailable: {error}"));
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
            Err(_) => {
                loading.set_visible(true);
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
    let custom: Vec<_> = sources
        .iter()
        .filter(|source| !CATALOGUE.iter().any(|entry| entry.matches(source)))
        .collect();
    if !custom.is_empty() {
        list.append(&label("Imported", "writy-library-title"));
        for source in custom {
            installed_row(list, source, tx, message);
        }
    }
    let explore = gtk::Box::new(gtk::Orientation::Vertical, 0);
    for entry in CATALOGUE {
        let source = sources.iter().find(|source| entry.matches(source));
        let installed = source.is_some();
        let busy = OPERATIONS.with(|active| active.borrow().get(entry.name).copied());
        let row = row(entry.name, entry.detail);
        if let Some(source) = source {
            row.append(&source_controls(source, tx, message));
        }
        let actions = gtk::FlowBox::new();
        actions.add_css_class("writy-dictionary-actions");
        actions.set_halign(gtk::Align::Start);
        actions.set_homogeneous(false);
        actions.set_selection_mode(gtk::SelectionMode::None);
        actions.set_min_children_per_line(1);
        actions.set_max_children_per_line(3);
        actions.set_column_spacing(4);
        actions.set_row_spacing(4);
        let download = components::action_button(if installed {
            "Download"
        } else {
            busy.unwrap_or("Download")
        });
        download.set_sensitive(!installed && busy.is_none());
        download.add_css_class("writy-download-action");
        download.update_property(&[gtk::accessible::Property::Label(&format!(
            "Download {}",
            entry.name
        ))]);
        let target = list.clone();
        let commands = tx.clone();
        let status = message.clone();
        download.connect_clicked(move |button| {
            install_pack(button, entry, &target, &commands, &status)
        });
        if !installed {
            actions.insert(&download, -1);
        }
        if installed {
            let remove = components::action_button(if busy == Some("Removing…") {
                "Removing…"
            } else {
                "Remove"
            });
            remove.set_sensitive(busy.is_none());
            remove.add_css_class("destructive-action");
            remove.update_property(&[gtk::accessible::Property::Label(&format!(
                "Remove {}",
                entry.name
            ))]);
            let target = list.clone();
            let commands = tx.clone();
            let status = message.clone();
            remove.connect_clicked(move |button| {
                remove_pack(button, entry, &target, &commands, &status)
            });
            actions.insert(&remove, -1);
        }
        actions.insert(
            &gtk::LinkButton::with_label(entry.url, "Source & licence"),
            -1,
        );
        for index in 0..2 {
            actions.child_at_index(index).unwrap().set_focusable(false);
        }
        row.append(&actions);
        explore.append(&row);
    }
    list.append(&explore);
    list.append(&label("Switch off to keep data without using it. Remove deletes the dictionary and cached download.", "writy-meta"));
}

fn row(name: &str, detail: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
    row.add_css_class("writy-dictionary-row");
    row.append(&label(name, "writy-title"));
    row.append(&label(detail, "writy-meta"));
    row
}
fn installed_row(list: &gtk::Box, source: &DictionarySource, tx: &Commands, message: &gtk::Label) {
    let row = row(&source.attribution.name, &source.attribution.license);
    row.append(&source_controls(source, tx, message));
    list.append(&row);
}
fn source_controls(source: &DictionarySource, tx: &Commands, message: &gtk::Label) -> gtk::Box {
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let status = label(
        &installed_status(source.entries, source.enabled),
        "writy-dictionary-state",
    );
    status.set_hexpand(true);
    controls.append(&status);
    let toggle = gtk::Switch::new();
    toggle.set_valign(gtk::Align::Center);
    toggle.set_active(source.enabled);
    let catalogue = CATALOGUE.iter().find(|entry| entry.matches(source));
    toggle.set_sensitive(
        !catalogue.is_some_and(|entry| {
            OPERATIONS.with(|active| active.borrow().contains_key(entry.name))
        }),
    );
    toggle.update_property(&[gtk::accessible::Property::Label(&format!(
        "Enable {}",
        source.attribution.name
    ))]);
    let id = source.id.clone();
    let count = source.entries;
    let tx = tx.clone();
    let message = message.clone();
    toggle.connect_state_set(move |_, enabled| {
        if catalogue
            .is_some_and(|entry| OPERATIONS.with(|active| active.borrow().contains_key(entry.name)))
        {
            return glib::Propagation::Stop;
        }
        match crate::ipc::open_dictionary().and_then(|d| d.set_source_enabled(&id, enabled)) {
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
    controls
}
fn installed_status(count: usize, enabled: bool) -> String {
    format!(
        "Installed · {} · {count} entries",
        if enabled { "Enabled" } else { "Disabled" }
    )
}
pub(super) fn install_pack(
    button: &gtk::Button,
    entry: &'static CatalogueEntry,
    list: &gtk::Box,
    tx: &Commands,
    message: &gtk::Label,
) {
    if !begin_operation(entry.name, "Downloading…") {
        return;
    }
    button.set_sensitive(false);
    button.set_label("Downloading…");
    message.set_text(&format!(
        "Downloading {}. You can keep using Writy.",
        entry.name
    ));
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| -> anyhow::Result<usize> {
            tokio::runtime::Runtime::new()?.block_on(
                DictionaryLibrary::new(crate::ipc::database_path()).install(entry, || {
                    let _ = sender.send(Ok(None));
                }),
            )
        })()
        .map_err(|error| format!("{error:#}"));
        let _ = sender.send(result.map(Some));
    });
    let button = button.clone();
    let list = list.clone();
    let tx = tx.clone();
    let message = message.clone();
    glib::timeout_add_local(Duration::from_millis(100), move || {
        match receiver.try_recv() {
            Ok(Ok(None)) => {
                OPERATIONS.with(|active| active.borrow_mut().insert(entry.name, "Importing…"));
                button.set_label("Importing…");
                message.set_text(&format!(
                    "Installing {}. Large dictionaries can take several minutes.",
                    entry.name
                ));
                return glib::ControlFlow::Continue;
            }
            Ok(Ok(Some(count))) => {
                OPERATIONS.with(|active| active.borrow_mut().remove(entry.name));
                message.set_text(&format!("{} installed: {count} entries.", entry.name));
                populate(&list, &tx, &message);
                send(&tx, Command::Reload);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
            result => {
                OPERATIONS.with(|active| active.borrow_mut().remove(entry.name));
                let error = match result {
                    Ok(Err(error)) => error,
                    _ => "Installation stopped. Try again.".into(),
                };
                button.set_sensitive(true);
                button.set_label("Download");
                button.set_tooltip_text(Some(&error));
                message.set_text(&format!("Installation failed: {error}"));
                populate(&list, &tx, &message);
            }
        }
        glib::ControlFlow::Break
    });
}

fn remove_pack(
    button: &gtk::Button,
    entry: &'static CatalogueEntry,
    list: &gtk::Box,
    tx: &Commands,
    message: &gtk::Label,
) {
    if !begin_operation(entry.name, "Removing…") {
        return;
    }
    button.set_sensitive(false);
    button.set_label("Removing…");
    message.set_text(&format!("Removing {}…", entry.name));
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = DictionaryLibrary::new(crate::ipc::database_path())
            .remove(entry)
            .map_err(|error| format!("{error:#}"));
        let _ = sender.send(result);
    });
    let list = list.clone();
    let tx = tx.clone();
    let message = message.clone();
    glib::timeout_add_local(Duration::from_millis(100), move || {
        let result = match receiver.try_recv() {
            Err(std::sync::mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
            Err(_) => Err("Dictionary removal stopped. Try again.".into()),
            Ok(result) => result,
        };
        OPERATIONS.with(|active| active.borrow_mut().remove(entry.name));
        match result {
            Ok(()) => {
                message.set_text(&format!("{} removed.", entry.name));
                send(&tx, Command::Reload);
            }
            Err(error) => message.set_text(&format!("Removal failed: {error}")),
        }
        populate(&list, &tx, &message);
        glib::ControlFlow::Break
    });
}

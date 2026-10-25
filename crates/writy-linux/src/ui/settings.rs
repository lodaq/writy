//! Grouped preferences with persistent actions and inline operation feedback.
use super::*;

impl View {
    pub(super) fn settings(&self) {
        if let Some(window) = self
            .app
            .windows()
            .into_iter()
            .find(|w| w.has_css_class("writy-settings"))
        {
            window.present();
            return;
        }
        let window = gtk::ApplicationWindow::builder()
            .application(&self.app)
            .title("Writy Settings")
            .default_width(620)
            .default_height(640)
            .build();
        window.add_css_class("writy-settings");
        window.set_transient_for(Some(&self.search));
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        root.set_margin_top(20);
        root.set_margin_bottom(20);
        root.set_margin_start(24);
        root.set_margin_end(24);
        let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let toolbar = gtk::HeaderBar::new();
        toolbar.add_css_class("writy-toolbar");
        toolbar.set_title_widget(Some(&label("Settings", "writy-title")));
        window.set_titlebar(Some(&toolbar));
        let (privacy_section, privacy) = section(
            "Privacy & lookup",
            "Choose how Writy helps while you write.",
        );
        root.append(&privacy_section);
        let loaded = Config::load().unwrap_or_else(|_| self.config.clone());
        let offline = gtk::CheckButton::with_label("Offline only — prevent cloud requests");
        offline.set_active(loaded.offline);
        privacy.append(&offline);
        let selection = gtk::CheckButton::with_label("Look up selected text automatically");
        selection.set_active(loaded.selection_popup);
        privacy.append(&selection);
        privacy.append(&label("When configured, online help sends text automatically. Hidden secure fields in opaque apps may not be detected. Terminal capture requires prose mode.","writy-status"));
        let (models_section, models) = section(
            "Language models",
            "Use installed Ollama models for writing and translation.",
        );
        root.append(&models_section);
        let advanced = gtk::Expander::new(Some("Cloud provider"));
        let cloud = gtk::Box::new(gtk::Orientation::Vertical, 12);
        cloud.set_margin_top(12);
        advanced.set_child(Some(&cloud));
        let fields = [
            ("Ollama URL", loaded.model.ollama_url.clone()),
            (
                "Writing, word details and live translation model",
                loaded.model.rewrite_model.clone(),
            ),
            (
                "Selection and search translation model",
                loaded.model.translation_model.clone(),
            ),
            (
                "Cloud base URL (including /v1)",
                loaded.model.cloud_url.clone(),
            ),
            ("Cloud model", loaded.model.cloud_model.clone()),
            (
                "API key environment variable",
                loaded.model.api_key_env.clone(),
            ),
        ];
        let mut entries = Vec::new();
        let mut model_choices = Vec::new();
        for (index, (title, value)) in fields.into_iter().enumerate() {
            let field = gtk::Box::new(gtk::Orientation::Vertical, 6);
            field.append(&label(title, "writy-meta"));
            if index < 3 {
                models.append(&field);
            } else {
                cloud.append(&field);
            }
            let e = gtk::Entry::new();
            e.update_property(&[gtk::accessible::Property::Label(title)]);
            e.set_text(&value);
            if matches!(index, 1 | 2) {
                let choices = gtk::DropDown::from_strings(&[value.as_str()]);
                choices.set_enable_search(true);
                choices.update_property(&[gtk::accessible::Property::Label(title)]);
                let entry = e.clone();
                choices.connect_selected_notify(move |dropdown| {
                    if let Some(item) = dropdown.selected_item().and_downcast::<gtk::StringObject>()
                    {
                        entry.set_text(&item.string());
                    }
                });
                e.set_placeholder_text(Some("Enter a model name"));
                field.append(&e);
                let picker = gtk::Expander::new(Some("Choose an installed model"));
                picker.set_child(Some(&choices));
                field.append(&picker);
                model_choices.push((choices, e.clone()));
            } else {
                field.append(&e);
            }
            entries.push(e);
        }
        let refresh = gtk::Button::with_label("Refresh installed Ollama models");
        let model_status = label("", "writy-status");
        models.append(&refresh);
        models.append(&model_status);
        models.append(&label("All installed tags are listed. Some models are for embeddings, OCR or cloud use rather than writing. Offline mode still blocks cloud-backed models.", "writy-status"));
        models.append(&advanced);
        let url = entries[0].clone();
        refresh.connect_clicked(move |button| {
            button.set_sensitive(false);
            model_status.set_text("Loading installed models…");
            let url = url.text().to_string();
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            std::thread::spawn(move || {
                let result = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|_| "Cannot start model discovery".to_owned())
                    .and_then(|runtime| {
                        runtime
                            .block_on(writy_core::provider::ollama_models(&url))
                            .map_err(|error| error.to_string())
                    });
                let _ = sender.send(result);
            });
            let choices = model_choices.clone();
            let button = button.clone();
            let status = model_status.clone();
            glib::timeout_add_local(Duration::from_millis(50), move || {
                match receiver.try_recv() {
                    Ok(Ok(names)) => {
                        for (dropdown, entry) in &choices {
                            let selected = entry.text().to_string();
                            let mut options = names.clone();
                            if !selected.is_empty() && !options.contains(&selected) {
                                options.push(selected.clone());
                            }
                            let index = options
                                .iter()
                                .position(|name| name == &selected)
                                .unwrap_or(0);
                            let refs: Vec<_> = options.iter().map(String::as_str).collect();
                            dropdown.set_model(Some(&gtk::StringList::new(&refs)));
                            dropdown.set_selected(index as u32);
                        }
                        status.set_text(&if names.is_empty() {
                            "No models installed in Ollama".into()
                        } else {
                            format!(
                                "{} installed models · choose above, then Save settings",
                                names.len()
                            )
                        });
                    }
                    Ok(Err(error)) => status.set_text(&error),
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        return glib::ControlFlow::Continue;
                    }
                    Err(_) => status.set_text("Model discovery stopped; try Refresh"),
                }
                button.set_sensitive(true);
                glib::ControlFlow::Break
            });
        });
        refresh.emit_clicked();
        let (panels_section, panel_options) = section(
            "Floating panels",
            "Choose which details appear beside your writing.",
        );
        root.append(&panels_section);
        let toggles = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let mut huds = Vec::new();
        for (key, title) in CATEGORIES {
            let c = gtk::CheckButton::with_label(title);
            c.set_active(loaded.huds.get(key).copied().unwrap_or(true));
            toggles.append(&c);
            huds.push((key, c));
        }
        panel_options.append(&toggles);
        let message = label("", "writy-status");
        let (dictionary_section, dictionaries) = section(
            "Offline dictionaries",
            "English ↔ Arabic lookups, available without a network after installation.",
        );
        root.append(&dictionary_section);
        let install = gtk::Button::with_label("Install English ↔ Arabic dictionaries");
        let msg = message.clone();
        install.connect_clicked(move |button| {
            let result = std::env::current_exe().and_then(|exe| {
                std::process::Command::new(exe)
                    .args(["dict", "install"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
            });
            match result {
                Ok(mut child) => {
                    button.set_sensitive(false);
                    msg.set_text("Downloading attributed FreeDict packs…");
                    let msg = msg.clone();
                    let b = button.clone();
                    glib::timeout_add_local(Duration::from_millis(500), move || {
                        match child.try_wait() {
                            Ok(Some(s)) => {
                                b.set_sensitive(true);
                                msg.set_text(if s.success() {
                                    "Dictionaries installed. Try a lookup."
                                } else {
                                    "Install failed. Run `writy dict install` for details."
                                });
                                glib::ControlFlow::Break
                            }
                            Ok(None) => glib::ControlFlow::Continue,
                            Err(_) => {
                                b.set_sensitive(true);
                                msg.set_text("Could not check installation. Try again.");
                                glib::ControlFlow::Break
                            }
                        }
                    });
                }
                Err(_) => msg.set_text("Cannot start dictionary installer"),
            }
        });
        dictionaries.append(&install);
        let save = gtk::Button::with_label("Save settings");
        save.add_css_class("suggested-action");
        let tx = self.tx.clone();
        let msg = message.clone();
        save.connect_clicked(move |_| {
            let mut c = loaded.clone();
            c.offline = offline.is_active();
            c.selection_popup = selection.is_active();
            c.model.ollama_url = entries[0].text().into();
            c.model.rewrite_model = entries[1].text().into();
            c.model.translation_model = entries[2].text().into();
            c.model.cloud_url = entries[3].text().into();
            c.model.cloud_model = entries[4].text().into();
            c.model.api_key_env = entries[5].text().into();
            for (key, toggle) in &huds {
                c.huds.insert((*key).into(), toggle.is_active());
            }
            match c.save() {
                Ok(()) => {
                    send(&tx, Command::Reload);
                    msg.set_text("Saved. Restart the daemon if selection watching changed.");
                }
                Err(e) => msg.set_text(&format!("Settings not saved: {e}")),
            }
        });
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        footer.add_css_class("writy-footer");
        message.set_hexpand(true);
        footer.append(&message);
        footer.append(&save);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&root)
            .build();
        shell.append(&scroll);
        shell.append(&footer);
        window.set_child(Some(&shell));
        let key = gtk::EventControllerKey::new();
        let weak = window.downgrade();
        key.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                if let Some(window) = weak.upgrade() {
                    window.close();
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(key);
        window.present();
    }
}

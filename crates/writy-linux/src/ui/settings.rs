//! Grouped preferences with persistent actions and inline operation feedback.
use super::*;

fn page(tabs: &gtk::Notebook, name: &str, title: &str, detail: &str) -> gtk::Box {
    let (section, content) = section(title, detail);
    section.set_margin_top(10);
    section.set_margin_bottom(10);
    section.set_margin_start(14);
    section.set_margin_end(14);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&section)
        .build();
    tabs.append_page(&scroll, Some(&components::tab_label(name)));
    content
}

fn toggle_row(title: &str, detail: &str, active: bool) -> (gtk::Box, gtk::Switch) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("writy-preference-row");
    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.append(&label(title, "writy-preference-title"));
    if !detail.is_empty() {
        text.append(&label(detail, "writy-meta"));
    }
    row.append(&text);
    let toggle = gtk::Switch::new();
    toggle.set_active(active);
    toggle.set_valign(gtk::Align::Center);
    toggle.update_property(&[gtk::accessible::Property::Label(title)]);
    row.append(&toggle);
    (row, toggle)
}

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
            .default_width(720)
            .default_height(640)
            .build();
        window.add_css_class("writy-settings");
        window.set_transient_for(Some(&self.search));
        let pages = gtk::Notebook::new();
        pages.add_css_class("writy-settings-tabs");
        pages.set_scrollable(true);
        pages.set_show_border(false);
        pages.set_vexpand(true);
        let shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let toolbar = gtk::HeaderBar::new();
        toolbar.add_css_class("writy-toolbar");
        let title = gtk::Label::new(Some("Settings"));
        title.add_css_class("writy-title");
        title.set_wrap(false);
        toolbar.set_title_widget(Some(&title));
        window.set_titlebar(Some(&toolbar));
        let loaded = Config::load().unwrap_or_else(|_| self.config.clone());
        let automatic = page(
            &pages,
            "Automatic assistance",
            "Automatic assistance",
            "Choose when Writy opens automatically. Changes apply when you save.",
        );
        let (row, selection) = toggle_row(
            "Show on selection",
            "Open the Text assistant when you select text in another app.",
            loaded.selection_popup,
        );
        automatic.append(&row);
        let (row, copied) = toggle_row(
            "Show on copy",
            "Open the Text assistant when you copy text in another app.",
            loaded.copy_popup,
        );
        automatic.append(&row);
        let (row, typing) = toggle_row(
            "Show while typing",
            "Analyze newly typed text and open the Text assistant. Manual tools remain available when off.",
            loaded.typing_popup,
        );
        automatic.append(&row);
        automatic.append(&label(
            "Typing pause before analysis (milliseconds)",
            "writy-meta",
        ));
        let idle = gtk::SpinButton::with_range(50.0, 10000.0, 50.0);
        idle.set_value(loaded.typing.idle_ms as f64);
        idle.update_property(&[gtk::accessible::Property::Label(
            "Typing pause before analysis (milliseconds)",
        )]);
        automatic.append(&idle);
        automatic.append(&label(
            "AI analysis waits for this pause and a final full stop. Local checks can run sooner.",
            "writy-meta",
        ));
        automatic.append(&label(
            "Allow assistance in this terminal",
            "writy-preference-title",
        ));
        automatic.append(&label("Use writy prose from the target terminal or its shortcut to toggle temporary permission. Commands and hidden prompts cannot be identified reliably.", "writy-meta"));
        automatic.append(&self.pause);
        let privacy = page(
            &pages,
            "Privacy",
            "Privacy",
            "Control online requests and excluded apps.",
        );
        let (row, offline) = toggle_row(
            "Offline only",
            "Prevent cloud requests. Local dictionaries and Ollama remain available.",
            loaded.offline,
        );
        privacy.append(&row);
        privacy.append(&label(
            "Excluded apps (one app name or ID per line)",
            "writy-meta",
        ));
        let excluded = gtk::TextView::new();
        excluded.set_wrap_mode(gtk::WrapMode::WordChar);
        excluded.set_size_request(-1, 90);
        excluded
            .buffer()
            .set_text(&loaded.apps.blocklist.join("\n"));
        excluded.update_property(&[gtk::accessible::Property::Label("Excluded apps")]);
        privacy.append(&excluded);
        privacy.append(&label("When configured, online help sends text automatically. Hidden secure fields in opaque apps may not be detected. Terminal assistance requires temporary permission.", "writy-status"));
        let models = page(
            &pages,
            "AI models",
            "AI models",
            "Use installed Ollama models for writing and translation.",
        );
        let translation = page(
            &pages,
            "Translation",
            "Translation engines",
            "Choose an engine. Google, Bing and Yandex work without API keys.",
        );
        let engines = [
            writy_core::config::TranslationEngine::Model,
            writy_core::config::TranslationEngine::Google,
            writy_core::config::TranslationEngine::Bing,
            writy_core::config::TranslationEngine::Yandex,
            writy_core::config::TranslationEngine::Apertium,
        ];
        translation.append(&label(
            "Selection and search translation engine",
            "writy-meta",
        ));
        let engine = gtk::DropDown::from_strings(&[
            "Language model",
            "Google",
            "Bing",
            "Yandex",
            "Apertium",
        ]);
        engine.update_property(&[gtk::accessible::Property::Label("Translation engine")]);
        engine.set_selected(
            engines
                .iter()
                .position(|e| *e == loaded.model.translation_engine)
                .unwrap_or(0) as u32,
        );
        translation.append(&engine);
        translation.append(&label("Select Google, Bing or Yandex and save to use keyless translation. These unofficial interfaces may be blocked or change. Offline only blocks online requests; failures use Ollama.", "writy-meta"));
        let mut provider_entries = Vec::new();
        for (title, key, detail, url) in [
            (
                "Google API key (optional)",
                loaded.model.google_api_key.clone(),
                "Leave blank for keyless translation. For the official API, create a Google Cloud project, enable billing and Cloud Translation API, then create an API key restricted to that API.",
                "https://docs.cloud.google.com/translate/docs/setup",
            ),
            (
                "Microsoft Translator (Bing) API key (optional)",
                loaded.model.bing_api_key.clone(),
                "Leave blank for keyless translation. For the official API, create an Azure Translator resource. Copy Key 1 from Resource Management → Keys and Endpoint. Use a public global endpoint resource.",
                "https://learn.microsoft.com/en-us/azure/ai-services/translator/how-to-create-translator-resource",
            ),
            (
                "Yandex API key (optional)",
                loaded.model.yandex_api_key.clone(),
                "Leave blank for keyless translation. For the official API, create a service account, grant ai.translate.user on its folder, then create an API key with yc.ai.translate.execute scope.",
                "https://yandex.cloud/en/docs/translate/api-ref/authentication",
            ),
        ] {
            let group = gtk::Expander::new(Some(title));
            group.set_label_widget(Some(&label(title, "writy-title")));
            let fields = gtk::Box::new(gtk::Orientation::Vertical, 4);
            let secret = gtk::PasswordEntry::new();
            secret.set_show_peek_icon(true);
            secret.set_width_chars(8);
            secret.set_text(&key);
            secret.update_property(&[gtk::accessible::Property::Label(title)]);
            fields.append(&secret);
            fields.append(&label(detail, "writy-meta"));
            fields.append(&gtk::LinkButton::with_label(url, "Setup guide"));
            group.set_child(Some(&fields));
            translation.append(&group);
            provider_entries.push(secret);
        }
        translation.append(&label(
            "Microsoft API region (optional; unused for keyless)",
            "writy-meta",
        ));
        let region = gtk::Entry::new();
        region.set_width_chars(8);
        region.set_text(&loaded.model.bing_region);
        region.set_placeholder_text(Some("e.g. eastus — shown in Keys and Endpoint"));
        region.update_property(&[gtk::accessible::Property::Label(
            "Microsoft Translator region",
        )]);
        translation.append(&region);
        translation.append(&label("Keys are saved as plain text in your private config file (owner read/write only). Keys in environment variables also select the official APIs; without a key, keyless translation is used. Click Save settings to apply; no restart needed. Apertium needs no key but currently has no English–Arabic pair.", "writy-meta"));
        let advanced = gtk::Expander::new(Some("Cloud AI provider"));
        advanced.set_label_widget(Some(&label("Cloud AI provider", "writy-meta")));
        let cloud = gtk::Box::new(gtk::Orientation::Vertical, 4);
        cloud.set_margin_top(7);
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
            let field = gtk::Box::new(gtk::Orientation::Vertical, 4);
            field.append(&label(title, "writy-meta"));
            if index < 3 {
                models.append(&field);
            } else {
                cloud.append(&field);
            }
            let e = gtk::Entry::new();
            e.set_width_chars(8);
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
                picker.set_label_widget(Some(&label("Choose an installed model", "writy-meta")));
                picker.set_child(Some(&choices));
                field.append(&picker);
                model_choices.push((choices, e.clone()));
            } else {
                field.append(&e);
            }
            entries.push(e);
        }
        let refresh = components::action_button("Refresh installed Ollama models");
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
        let panel_options = page(
            &pages,
            "Text assistant",
            "Result sections",
            "Choose which details appear beside your writing.",
        );
        let toggles = gtk::Box::new(gtk::Orientation::Vertical, 1);
        let mut huds = Vec::new();
        for (key, title) in CATEGORIES {
            let (row, c) = toggle_row(title, "", loaded.huds.get(key).copied().unwrap_or(true));
            toggles.append(&row);
            huds.push((key, c));
        }
        panel_options.append(&toggles);
        let message = label("", "writy-status");
        let dictionaries = page(
            &pages,
            "Dictionaries",
            "Dictionary library",
            "Download, enable or remove dictionaries.",
        );
        let catalogue = gtk::Box::new(gtk::Orientation::Vertical, 4);
        dictionaries.append(&catalogue);
        super::dictionaries::populate(&catalogue, &self.tx, &message);
        super::dictionaries::watch_installs(&catalogue, &self.tx, &message);
        let refresh_dictionaries = components::action_button("Refresh dictionary status");
        let list = catalogue.clone();
        let dictionary_tx = self.tx.clone();
        let dictionary_message = message.clone();
        refresh_dictionaries.connect_clicked(move |_| {
            super::dictionaries::populate(&list, &dictionary_tx, &dictionary_message)
        });
        dictionaries.append(&refresh_dictionaries);
        let save = components::action_button("Save settings");
        save.add_css_class("suggested-action");
        let tx = self.tx.clone();
        let msg = message.clone();
        save.connect_clicked(move |_| {
            let mut c = loaded.clone();
            c.offline = offline.is_active();
            c.selection_popup = selection.is_active();
            c.copy_popup = copied.is_active();
            c.typing_popup = typing.is_active();
            c.typing.idle_ms = idle.value_as_int() as u64;
            c.apps.blocklist = excluded
                .buffer()
                .text(
                    &excluded.buffer().start_iter(),
                    &excluded.buffer().end_iter(),
                    false,
                )
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect();
            c.model.ollama_url = entries[0].text().into();
            c.model.rewrite_model = entries[1].text().into();
            c.model.translation_model = entries[2].text().into();
            c.model.translation_engine = engines[engine.selected() as usize];
            c.model.google_api_key = provider_entries[0].text().into();
            c.model.bing_api_key = provider_entries[1].text().into();
            c.model.yandex_api_key = provider_entries[2].text().into();
            c.model.bing_region = region.text().trim().into();
            c.model.cloud_url = entries[3].text().into();
            c.model.cloud_model = entries[4].text().into();
            c.model.api_key_env = entries[5].text().into();
            for (key, toggle) in &huds {
                c.huds.insert((*key).into(), toggle.is_active());
            }
            match c.save() {
                Ok(()) => {
                    send(&tx, Command::Reload);
                    msg.set_text("Saved. Settings applied; no restart needed.");
                }
                Err(e) => msg.set_text(&format!("Settings not saved: {e}")),
            }
        });
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 7);
        footer.add_css_class("writy-footer");
        message.set_hexpand(true);
        footer.append(&message);
        footer.append(&save);
        components::enable_wheel_tab_switching(&pages);
        shell.append(&pages);
        shell.append(&footer);
        window.set_child(Some(&shell));
        window.connect_close_request(|window| {
            window.set_visible(false);
            glib::Propagation::Stop
        });
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

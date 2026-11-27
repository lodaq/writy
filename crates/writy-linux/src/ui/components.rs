//! Shared native components for search, settings and floating panels.
use super::*;

glib::wrapper! {
    pub struct ReadingColumn(ObjectSubclass<reading_column::ReadingColumn>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl ReadingColumn {
    pub fn new(
        child: &gtk::Widget,
        actions: &gtk::Box,
        header: &gtk::Box,
        split: &gtk::Box,
    ) -> Self {
        use gtk::subclass::prelude::ObjectSubclassIsExt;
        let column: Self = glib::Object::new();
        column.imp().child.set(child.clone()).unwrap();
        column.imp().actions.set(actions.clone()).unwrap();
        column.imp().header.set(header.clone()).unwrap();
        column.imp().split.set(split.clone()).unwrap();
        child.set_parent(&column);
        column
    }
}

mod reading_column {
    use super::*;
    use gtk::subclass::prelude::*;
    use std::cell::OnceCell;

    #[derive(Default)]
    pub struct ReadingColumn {
        pub child: OnceCell<gtk::Widget>,
        pub actions: OnceCell<gtk::Box>,
        pub header: OnceCell<gtk::Box>,
        pub split: OnceCell<gtk::Box>,
    }
    fn inset(width: i32) -> i32 {
        (width / 60).clamp(8, 16)
    }
    impl ReadingColumn {
        fn arrange_actions(&self, width: i32) {
            if let Some(split) = self.split.get() {
                split.set_orientation(if width >= 1000 {
                    gtk::Orientation::Horizontal
                } else {
                    gtk::Orientation::Vertical
                });
            }
            for row in [self.actions.get(), self.header.get()]
                .into_iter()
                .flatten()
            {
                row.set_orientation(if width < 720 {
                    gtk::Orientation::Vertical
                } else {
                    gtk::Orientation::Horizontal
                });
            }
        }
    }
    #[glib::object_subclass]
    impl ObjectSubclass for ReadingColumn {
        const NAME: &'static str = "WrityReadingColumn";
        type Type = super::ReadingColumn;
        type ParentType = gtk::Widget;
        fn class_init(class: &mut Self::Class) {
            class.set_accessible_role(gtk::AccessibleRole::Group);
        }
    }
    impl ObjectImpl for ReadingColumn {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }
    impl WidgetImpl for ReadingColumn {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let Some(child) = self.child.get() else {
                return (0, 0, -1, -1);
            };
            let size = if orientation == gtk::Orientation::Vertical && for_size >= 0 {
                let width = for_size - 2 * inset(for_size);
                self.arrange_actions(width);
                width
            } else {
                for_size
            };
            let (minimum, natural, min_baseline, nat_baseline) = child.measure(orientation, size);
            if orientation == gtk::Orientation::Horizontal {
                (
                    minimum.min(320) + 40,
                    (natural + 160).max(minimum + 40),
                    min_baseline,
                    nat_baseline,
                )
            } else {
                (minimum, natural, min_baseline, nat_baseline)
            }
        }
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(child) = self.child.get() {
                let padding = inset(width);
                let child_width = width - 2 * padding;
                self.arrange_actions(child_width);
                let position = gtk::graphene::Point::new(padding as f32, 0.);
                child.allocate(
                    child_width,
                    height,
                    baseline,
                    Some(gtk::gsk::Transform::new().translate(&position)),
                );
            }
        }
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            if let Some(child) = self.child.get() {
                self.obj().snapshot_child(child, snapshot);
            }
        }
    }
}

pub(super) fn section(title: &str, description: &str) -> (gtk::Box, gtk::Box) {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 3);
    let heading = label(title, "writy-title");
    heading.add_css_class("writy-section-title");
    section.append(&heading);
    if !description.is_empty() {
        section.append(&label(description, "writy-meta"));
    }
    let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
    content.add_css_class("writy-card");
    section.append(&content);
    (section, content)
}
pub(super) fn empty_state(title: &str, detail: &str) -> gtk::Box {
    let state = gtk::Box::new(gtk::Orientation::Vertical, 3);
    state.add_css_class("writy-empty");
    state.set_valign(gtk::Align::Center);
    state.set_vexpand(true);
    let icon = gtk::Image::from_icon_name("edit-find-symbolic");
    icon.set_pixel_size(32);
    state.append(&icon);
    let heading = label(title, "writy-empty-title");
    heading.set_xalign(0.5);
    state.append(&heading);
    let detail = label(detail, "writy-meta");
    detail.set_xalign(0.5);
    detail.set_justify(gtk::Justification::Center);
    state.append(&detail);
    state
}
pub(super) fn tab_label(caption: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(caption));
    label.set_wrap(false);
    label.set_tooltip_text(Some(caption));
    label
}

pub(super) fn enable_wheel_tab_switching(tabs: &gtk::Notebook) {
    let notebook = tabs.downgrade();
    let wheel = gtk::EventControllerScroll::new(
        gtk::EventControllerScrollFlags::BOTH_AXES | gtk::EventControllerScrollFlags::DISCRETE,
    );
    wheel.set_propagation_phase(gtk::PropagationPhase::Capture);
    wheel.connect_scroll(move |_, dx, dy| {
        if dx == 0. && dy == 0. {
            return glib::Propagation::Proceed;
        }
        if let Some(tabs) = notebook.upgrade() {
            switch_tab(&tabs, if dy != 0. { dy } else { dx });
        }
        glib::Propagation::Stop
    });
    // The header receives events over both tab labels and tab padding.
    tabs.first_child().unwrap().add_controller(wheel);
}

pub(super) fn switch_tab(tabs: &gtk::Notebook, delta: f64) {
    if delta == 0. {
        return;
    }
    let count = tabs.n_pages() as i32;
    let current = tabs.current_page().unwrap_or(0) as i32;
    let step = if delta > 0. { 1 } else { -1 };
    for offset in 1..=count {
        let page = (current + step * offset).rem_euclid(count) as u32;
        if tabs.nth_page(Some(page)).is_some_and(|w| w.is_visible()) {
            tabs.set_current_page(Some(page));
            break;
        }
    }
}

pub(super) fn action_button(caption: &str) -> gtk::Button {
    let button = gtk::Button::with_label(caption);
    let text = button.child().unwrap().downcast::<gtk::Label>().unwrap();
    text.set_wrap(true);
    text.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    text.set_justify(gtk::Justification::Center);
    button
}
pub(super) fn copy_button(text: Rc<RefCell<String>>, caption: &'static str) -> gtk::Button {
    let button = action_button(caption);
    button.set_halign(gtk::Align::Start);
    button.connect_clicked(move |button| {
        if text.borrow().is_empty() {
            return;
        }
        clipboard(&text.borrow());
        button.set_label("Copied");
        let weak = button.downgrade();
        glib::timeout_add_local_once(Duration::from_millis(1400), move || {
            if let Some(button) = weak.upgrade() {
                button.set_label(caption);
            }
        });
    });
    button
}
pub(super) fn result_card(panel: &Panel) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 3);
    card.add_css_class("writy-card");
    card.add_css_class("writy-result-section");
    let header = gtk::FlowBox::new();
    header.add_css_class("writy-result-header");
    header.set_selection_mode(gtk::SelectionMode::None);
    header.set_min_children_per_line(1);
    header.set_max_children_per_line(2);
    header.set_column_spacing(12);
    header.set_row_spacing(4);
    let title = label(panel.title, "writy-title");
    title.set_wrap_mode(gtk::pango::WrapMode::Word);
    title.set_hexpand(true);
    header.insert(&title, -1);
    let copy = copy_button(Rc::new(RefCell::new(panel.copy_text.clone())), "Copy");
    copy.set_sensitive(!panel.copy_text.is_empty());
    copy.set_tooltip_text(Some(&format!("Copy {}", panel.title.to_lowercase())));
    copy.set_halign(gtk::Align::End);
    header.insert(&copy, -1);
    for index in 0..2 {
        header.child_at_index(index).unwrap().set_focusable(false);
    }
    card.append(&header);
    if panel.groups.is_empty() {
        card.append(&label(&panel.body, "writy-status"));
    }
    for group in &panel.groups {
        let source = gtk::Box::new(gtk::Orientation::Vertical, 2);
        source.add_css_class("writy-source-group");
        source.append(&label(&group.source, "writy-source-title"));
        for (index, row) in group.rows.iter().enumerate() {
            let entry = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            entry.add_css_class("writy-result-entry");
            let (text, markup) = panel.search_row(group, row);
            let body = label(&text, "writy-body");
            body.set_selectable(true);
            body.set_focusable(false);
            if let Some(markup) = markup {
                body.set_markup(markup);
            }
            entry.set_direction(body.direction());
            if group.rows.len() > 1 {
                let number = label(&format!("{}.", index + 1), "writy-sense-number");
                number.set_xalign(1.);
                number.set_valign(gtk::Align::Start);
                entry.append(&number);
            }
            let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
            content.set_hexpand(true);
            if panel.category == "definition"
                && !row.headword.is_empty()
                && (index == 0
                    || row.headword != group.rows[index - 1].headword
                    || row.part_of_speech != group.rows[index - 1].part_of_speech)
            {
                let heading = gtk::Box::new(gtk::Orientation::Horizontal, 5);
                heading.set_direction(entry.direction());
                let headword = label(&row.headword, "writy-entry-headword");
                let leading = gtk::pango::AttrList::new();
                leading.insert(gtk::pango::AttrFloat::new_line_height(0.65));
                headword.set_attributes(Some(&leading));
                heading.append(&headword);
                if !row.part_of_speech.is_empty() && !row.text.starts_with(&row.part_of_speech) {
                    heading.append(&label(&row.part_of_speech, "writy-meta"));
                }
                content.append(&heading);
            }
            content.append(&body);
            entry.append(&content);
            source.append(&entry);
        }
        card.append(&source);
    }
    card
}

pub(super) fn hud_copy_button(text: Rc<RefCell<String>>, category: &'static str) -> gtk::Button {
    let button = gtk::Button::new();
    let icons = gtk::Stack::new();
    icons.set_transition_duration(125);
    icons.add_named(
        &gtk::Image::from_icon_name("edit-copy-symbolic"),
        Some("copy"),
    );
    icons.add_named(
        &gtk::Image::from_icon_name("object-select-symbolic"),
        Some("copied"),
    );
    icons.set_visible_child_name("copy");
    if let Some(settings) = gtk::Settings::default() {
        icons.set_transition_type(if settings.is_gtk_enable_animations() {
            gtk::StackTransitionType::Crossfade
        } else {
            gtk::StackTransitionType::None
        });
        let weak = icons.downgrade();
        settings.connect_gtk_enable_animations_notify(move |settings| {
            if let Some(icons) = weak.upgrade() {
                icons.set_transition_type(if settings.is_gtk_enable_animations() {
                    gtk::StackTransitionType::Crossfade
                } else {
                    gtk::StackTransitionType::None
                });
            }
        });
    }
    button.set_child(Some(&icons));
    let caption = format!("Copy {}", category.to_lowercase());
    button.set_tooltip_text(Some(&caption));
    button.update_property(&[gtk::accessible::Property::Label(&caption)]);
    // Repeated clicks retarget the same icon stack; never replay an entrance.
    let pending = Rc::new(RefCell::new(None::<glib::SourceId>));
    button.connect_clicked(move |button| {
        if text.borrow().is_empty() {
            return;
        }
        if let Some(timer) = pending.borrow_mut().take() {
            timer.remove();
        }
        clipboard(&text.borrow());
        icons.set_visible_child_name("copied");
        button.set_tooltip_text(Some("Copied"));
        button.update_property(&[gtk::accessible::Property::Label("Copied")]);
        let weak = button.downgrade();
        let icon_stack = icons.downgrade();
        let timer = pending.clone();
        let reset_caption = caption.clone();
        *pending.borrow_mut() = Some(glib::timeout_add_local_once(
            Duration::from_millis(1400),
            move || {
                timer.borrow_mut().take();
                if let (Some(button), Some(icons)) = (weak.upgrade(), icon_stack.upgrade()) {
                    icons.set_visible_child_name("copy");
                    button.set_tooltip_text(Some(&reset_caption));
                    button.update_property(&[gtk::accessible::Property::Label(&reset_caption)]);
                }
            },
        ));
    });
    button
}

// Keep the first label stable so unchanged content retains its text selection.
pub(super) fn response_rows(
    source: &gtk::Label,
    body: &gtk::Label,
    responses: &gtk::Box,
    panel: &Panel,
) {
    source.set_text(
        panel
            .groups
            .first()
            .map(|group| group.source.as_str())
            .unwrap_or_default(),
    );
    source.set_visible(!panel.groups.is_empty());
    if let Some(group) = panel.groups.first() {
        group_body(body, group);
    } else if body.text().as_str() != panel.body {
        body.set_text(&panel.body);
    }
    while let Some(child) = responses.first_child() {
        responses.remove(&child);
    }
    for group in panel.groups.iter().skip(1) {
        responses.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        responses.append(&label(&group.source, "writy-source-title"));
        let text = label("", "writy-body");
        group_body(&text, group);
        text.set_selectable(true);
        text.set_focusable(false);
        responses.append(&text);
    }
}

fn group_body(body: &gtk::Label, group: &SourceGroup) {
    match group.body() {
        DisplayText::Markup(markup) => {
            if body.label().as_str() != markup {
                body.set_markup(&markup);
            }
        }
        DisplayText::Plain(text) => {
            if body.text().as_str() != text {
                body.set_text(&text);
            }
        }
    }
}

fn kind_accent_class(kind: SuggestionKind) -> &'static str {
    match kind {
        SuggestionKind::Spelling => "writy-lint-spelling",
        SuggestionKind::Grammar => "writy-lint-grammar",
        SuggestionKind::Rewrite => "writy-lint-improvement",
    }
}

fn kind_colors(kind: SuggestionKind) -> (&'static str, &'static str) {
    match kind {
        SuggestionKind::Spelling => ("#f43f5e", "#f43f5e24"),
        SuggestionKind::Grammar => ("#a855f7", "#a855f724"),
        SuggestionKind::Rewrite => ("#0ea5e9", "#0ea5e921"),
    }
}

fn highlighted_preview_markup(
    text: &str,
    suggestions: &[writy_core::types::Suggestion],
) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    if len == 0 {
        return None;
    }
    let scoped: Vec<&writy_core::types::Suggestion> = suggestions
        .iter()
        .filter(|s| {
            s.start < s.end
                && s.end <= len
                && s.description != "Synonym; choose one that fits your meaning"
                && !(s.start == 0 && s.end == len && s.rule.is_empty())
        })
        .collect();
    if scoped.is_empty() {
        return None;
    }

    let rank = |kind: SuggestionKind| match kind {
        SuggestionKind::Spelling => 1u8,
        SuggestionKind::Grammar => 2,
        SuggestionKind::Rewrite => 3,
    };

    let mut per_char: Vec<Option<SuggestionKind>> = vec![None; len];
    for s in scoped {
        for slot in &mut per_char[s.start..s.end] {
            let replace = match slot {
                None => true,
                Some(existing) => rank(s.kind) < rank(*existing),
            };
            if replace {
                *slot = Some(s.kind);
            }
        }
    }

    let mut markup = String::new();
    let mut i = 0;
    while i < len {
        let current_kind = per_char[i];
        let mut j = i + 1;
        while j < len && per_char[j] == current_kind {
            j += 1;
        }
        let segment: String = chars[i..j].iter().collect();
        let escaped = glib::markup_escape_text(&segment);
        if let Some(kind) = current_kind {
            let (ul, bg) = kind_colors(kind);
            markup.push_str(&format!(
                "<span underline=\"single\" underline_color=\"{ul}\" background=\"{bg}\">{escaped}</span>"
            ));
        } else {
            markup.push_str(&escaped);
        }
        i = j;
    }
    Some(markup)
}

/// Search edits its own input; floating choices keep the verified external-edit path.
pub(super) fn improvements(
    result: &Results,
    tx: &Commands,
    editor: Option<&gtk::TextView>,
) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    if editor.is_some() {
        card.add_css_class("writy-result-section");
        card.append(&label("Suggestions", "writy-title"));
    }
    if result.suggestions.is_empty() {
        card.append(&label(
            if result.status == "Generating…" {
                "Finding suggestions…"
            } else {
                "No suggestions found. Try another word or phrase."
            },
            "writy-meta",
        ));
        return card;
    }

    if editor.is_none()
        && let Some(markup) = highlighted_preview_markup(&result.snapshot.text, &result.suggestions)
    {
        let preview_box = gtk::Box::new(gtk::Orientation::Vertical, 3);
        preview_box.add_css_class("writy-lint-hud-preview");
        let preview = label("", "writy-body");
        preview.set_markup(&markup);
        preview.set_selectable(true);
        preview.set_focusable(false);
        preview_box.append(&preview);
        card.append(&preview_box);
    }

    let snapshot_chars: Vec<char> = result.snapshot.text.chars().collect();
    for suggestion in &result.suggestions {
        let Some(replacement) = suggestion.replaced_text() else {
            continue;
        };
        let original_excerpt: String =
            if suggestion.start <= suggestion.end && suggestion.end <= snapshot_chars.len() {
                snapshot_chars[suggestion.start..suggestion.end]
                    .iter()
                    .collect()
            } else {
                String::new()
            };
        let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
        if editor.is_none() {
            row.add_css_class("writy-lint-hud-card");
        }

        if editor.is_none() {
            let head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let bar = gtk::Box::new(gtk::Orientation::Vertical, 0);
            bar.add_css_class("writy-lint-bar");
            bar.add_css_class(kind_accent_class(suggestion.kind));
            head.append(&bar);

            let cat = if !suggestion.category.is_empty() {
                suggestion.category.as_str()
            } else {
                match suggestion.kind {
                    SuggestionKind::Spelling => "Spelling",
                    SuggestionKind::Grammar => "Grammar",
                    SuggestionKind::Rewrite => "Improvement",
                }
            };
            let cat_label = label(cat, "writy-lint-category");
            cat_label.set_wrap(false);
            head.append(&cat_label);

            if !original_excerpt.is_empty() && original_excerpt != result.snapshot.text {
                let excerpt = label(&format!("“{original_excerpt}”"), "writy-meta");
                excerpt.set_wrap(false);
                excerpt.set_ellipsize(gtk::pango::EllipsizeMode::End);
                excerpt.set_hexpand(true);
                head.append(&excerpt);
            } else {
                let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                spacer.set_hexpand(true);
                head.append(&spacer);
            }

            let provider = label(&suggestion.provider, "writy-meta");
            provider.set_wrap(false);
            head.append(&provider);
            row.append(&head);

            let desc = label("", "writy-lint-message");
            desc.set_markup(&editor_lint::format_message_markup(&suggestion.description));
            row.append(&desc);

            let choices = suggestion.choices();
            let verified = result.snapshot.confidence == writy_core::types::Confidence::Verified;
            if !choices.is_empty() {
                let choice_box = gtk::FlowBox::new();
                choice_box.set_halign(gtk::Align::Start);
                choice_box.set_selection_mode(gtk::SelectionMode::None);
                choice_box.set_min_children_per_line(1);
                choice_box.set_max_children_per_line(4);
                choice_box.set_column_spacing(5);
                choice_box.set_row_spacing(4);
                for (idx, choice_text) in choices.iter().take(6).enumerate() {
                    let caption = if choice_text.is_empty() {
                        "Remove".to_owned()
                    } else if choices.len() == 1 && verified {
                        format!("Apply: {choice_text}")
                    } else {
                        (*choice_text).to_owned()
                    };
                    let btn = action_button(&caption);
                    btn.add_css_class("writy-lint-hud-choice");
                    if idx == 0 && verified {
                        btn.add_css_class("suggested-action");
                    }
                    btn.set_focusable(false);
                    if verified {
                        btn.set_tooltip_text(Some("Apply this fix directly to the active app"));
                        let tx = tx.clone();
                        let generation = result.generation;
                        let mut chosen = suggestion.clone();
                        chosen.replacement = (*choice_text).to_owned();
                        btn.connect_clicked(move |_| {
                            send(
                                &tx,
                                Command::Apply {
                                    generation,
                                    suggestion: chosen.clone(),
                                },
                            );
                        });
                    } else {
                        let fixed = suggestion.replaced_with(choice_text).unwrap_or_default();
                        btn.set_tooltip_text(Some(
                            "Direct edit unavailable for this app; click to copy fixed text",
                        ));
                        btn.connect_clicked(move |button| {
                            if fixed.is_empty() {
                                return;
                            }
                            clipboard(&fixed);
                            let prev = button.label().map(|s| s.to_string()).unwrap_or_default();
                            button.set_label("Copied ✓");
                            let weak = button.downgrade();
                            glib::timeout_add_local_once(Duration::from_millis(1200), move || {
                                if let Some(b) = weak.upgrade() {
                                    b.set_label(&prev);
                                }
                            });
                        });
                    }
                    choice_box.insert(&btn, -1);
                }
                for i in 0..choices.len().min(6) {
                    if let Some(child) = choice_box.child_at_index(i as i32) {
                        child.set_focusable(false);
                    }
                }
                row.append(&choice_box);
            }
        } else {
            let text = label(&replacement, "writy-body");
            text.set_selectable(true);
            row.append(&text);
            row.append(&label(
                &format!("{} · {}", suggestion.description, suggestion.provider),
                "writy-meta",
            ));
        }

        let actions = gtk::FlowBox::new();
        actions.set_halign(gtk::Align::Start);
        actions.set_selection_mode(gtk::SelectionMode::None);
        actions.set_min_children_per_line(1);
        actions.set_max_children_per_line(5);
        actions.set_column_spacing(5);
        actions.set_row_spacing(4);

        if let Some(editor) = editor {
            let apply = action_button("Apply");
            apply.set_focusable(true);
            let source = suggestion.snapshot.text.clone();
            let editor = editor.clone();
            let start = suggestion.start;
            let end = suggestion.end;
            let edit = suggestion.replacement.clone();
            apply.connect_clicked(move |button| {
                let buffer = editor.buffer();
                let current = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
                if current.trim() == source {
                    let offset = current.chars().count() - current.trim_start().chars().count();
                    let mut begin = buffer.iter_at_offset((offset + start) as i32);
                    let mut end = buffer.iter_at_offset((offset + end) as i32);
                    buffer.begin_user_action();
                    buffer.delete(&mut begin, &mut end);
                    buffer.insert(&mut begin, &edit);
                    buffer.end_user_action();
                    editor.grab_focus();
                } else {
                    button.set_sensitive(false);
                    button.set_tooltip_text(Some("Text changed; choose a current suggestion"));
                }
            });
            actions.insert(&apply, -1);
        }

        let copy = copy_button(
            Rc::new(RefCell::new(suggestion.replaced_text().unwrap_or_default())),
            "Copy",
        );
        copy.set_focusable(editor.is_some());
        actions.insert(&copy, -1);

        let dismiss = action_button("Dismiss");
        dismiss.set_focusable(editor.is_some());
        let dismiss_tx = tx.clone();
        let generation = result.generation;
        let choice = suggestion.clone();
        let dismissed_row = row.downgrade();
        dismiss.connect_clicked(move |_| {
            if let Some(row) = dismissed_row.upgrade() {
                row.set_visible(false);
            }
            send(
                &dismiss_tx,
                Command::DismissSuggestion {
                    generation,
                    suggestion: choice.clone(),
                },
            );
        });
        actions.insert(&dismiss, -1);

        if suggestion.rule == "SpellCheck"
            && suggestion.kind == SuggestionKind::Spelling
            && writy_core::types::is_single_word(&original_excerpt)
        {
            let add_dict = action_button("+ Dict");
            add_dict.set_focusable(editor.is_some());
            add_dict.set_tooltip_text(Some(&format!(
                "Add \"{original_excerpt}\" to Harper dictionary"
            )));
            let dict_tx = tx.clone();
            let word = original_excerpt.clone();
            let row_weak = row.downgrade();
            add_dict.connect_clicked(move |_| {
                if let Some(r) = row_weak.upgrade() {
                    r.set_visible(false);
                }
                send(&dict_tx, Command::HarperAddWord { word: word.clone() });
            });
            actions.insert(&add_dict, -1);
        }

        if !suggestion.rule.is_empty() {
            let disable = action_button(&format!("Disable {}", suggestion.rule));
            disable.add_css_class("flat");
            disable.set_focusable(editor.is_some());
            let rule_tx = tx.clone();
            let rule = suggestion.rule.clone();
            let row_weak = row.downgrade();
            disable.connect_clicked(move |_| {
                if let Some(r) = row_weak.upgrade() {
                    r.set_visible(false);
                }
                send(&rule_tx, Command::HarperDisableRule { rule: rule.clone() });
            });
            actions.insert(&disable, -1);
        }

        for index in 0..5 {
            if let Some(child) = actions.child_at_index(index) {
                child.set_focusable(false);
            }
        }
        row.append(&actions);
        card.append(&row);
    }
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use writy_core::types::{Confidence, Language, Suggestion, TextSnapshot};

    #[test]
    fn hud_preview_highlights_mistakes_and_improvements() {
        let text = "Wether i know very unique things.";
        let snapshot = TextSnapshot {
            source: "selection".into(),
            focus_epoch: 1,
            revision: 1,
            language: Language::English,
            text: text.into(),
            confidence: Confidence::Projected,
        };
        let suggestions = vec![
            Suggestion {
                snapshot: snapshot.clone(),
                start: 0,
                end: 6,
                replacement: "Whether".into(),
                description: "Spelling".into(),
                kind: SuggestionKind::Spelling,
                provider: "Harper · local".into(),
                alternatives: vec!["Whether".into(), "Weather".into()],
                rule: "SpellCheck".into(),
                category: "Spelling".into(),
                ignore_hash: Some(1),
            },
            Suggestion {
                snapshot,
                start: 14,
                end: 25,
                replacement: "very special".into(),
                description: "Style".into(),
                kind: SuggestionKind::Rewrite,
                provider: "Harper · local".into(),
                alternatives: vec!["very special".into()],
                rule: "VeryUnique".into(),
                category: "Usage".into(),
                ignore_hash: Some(2),
            },
        ];
        let markup = highlighted_preview_markup(text, &suggestions).expect("preview markup");
        assert!(markup.contains("#f43f5e"));
        assert!(markup.contains("Wether"));
        assert!(markup.contains("#0ea5e9"));
        assert!(markup.contains("very unique"));
    }
}

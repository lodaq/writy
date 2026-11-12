use crate::{
    backend::Commands,
    protocol::{Command, Results, UiEvent},
};
use gtk::{gdk, gio, glib, prelude::*};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::BTreeMap,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::Receiver,
    },
    time::Duration,
};
use writy_core::{config::Config, types::SuggestionKind};

mod components;
mod dictionaries;
mod hud;
mod settings;
use components::{copy_button, empty_state, result_card, section};

const CSS: &str = include_str!("ui.css");
fn layer_shell_supported() -> bool {
    // gtk-layer-shell only accepts Wayland displays; GTK can also select X11.
    gdk::Display::default().is_some_and(|display| {
        display.type_().name() == "GdkWaylandDisplay" && gtk4_layer_shell::is_supported()
    })
}
fn send(tx: &Commands, command: Command) {
    let _ = tx.try_send((command, None));
}
fn label(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    align_label(&l);
    l.connect_label_notify(align_label);
    l.set_wrap(true);
    l.set_max_width_chars(76);
    l.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    l.add_css_class(class);
    l
}
// Apply direction to content rather than the surrounding controls. Mixed
// dictionary lines containing Arabic also read from the right edge.
fn align_label(label: &gtk::Label) {
    let arabic = label.text().chars().any(|c| {
        matches!(c as u32,
        0x0600..=0x06ff | 0x0750..=0x077f | 0x08a0..=0x08ff |
        0xfb50..=0xfdff | 0xfe70..=0xfeff | 0x1ee00..=0x1eeff)
    });
    label.set_xalign(0.);
    if arabic {
        label.add_css_class("writy-rtl");
    } else {
        label.remove_css_class("writy-rtl");
    }
    // Pango mirrors logical alignment for RTL; Right would flip it twice.
    label.set_justify(gtk::Justification::Left);
    label.set_direction(if arabic {
        gtk::TextDirection::Rtl
    } else {
        gtk::TextDirection::Ltr
    });
}
fn clipboard(text: &str) {
    if let Some(display) = gdk::Display::default() {
        display.clipboard().set_text(text);
    }
}
const CATEGORIES: [(&str, &str); 7] = [
    ("translation", "Translation"),
    ("suggestions", "Writing"),
    ("definition", "Definition"),
    ("collocations", "Collocations"),
    ("examples", "Examples"),
    ("synonyms", "Synonyms & antonyms"),
    ("word_family", "Word family"),
];
#[derive(Clone, Debug, PartialEq)]
struct ResultRow {
    headword: String,
    part_of_speech: String,
    text: String,
    markup: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
struct SourceGroup {
    source: String,
    rows: Vec<ResultRow>,
}
struct Panel {
    category: &'static str,
    title: &'static str,
    body: String,
    groups: Vec<SourceGroup>,
    copy_text: String,
}
fn compact_arramooz(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .replace("ثلاثي\nلازم", "ثلاثي · لازم")
        .replace("ثلاثي\nمتعد", "ثلاثي · متعد")
}
fn panels(result: &Results) -> Vec<Panel> {
    let mut content: BTreeMap<&str, Vec<SourceGroup>> = BTreeMap::new();
    let mut add = |key, source: &str, row: ResultRow| {
        if row.text.trim().is_empty() {
            return;
        }
        let groups = content.entry(key).or_default();
        if let Some(group) = groups.iter_mut().find(|group| group.source == source) {
            if !group.rows.iter().any(|existing| {
                existing.headword == row.headword
                    && existing.part_of_speech == row.part_of_speech
                    && existing.text.trim() == row.text.trim()
            }) {
                group.rows.push(row);
            }
        } else {
            groups.push(SourceGroup {
                source: source.into(),
                rows: vec![row],
            });
        }
    };
    if let Some((text, provider)) = &result.translation {
        add(
            "translation",
            provider,
            ResultRow {
                headword: String::new(),
                part_of_speech: String::new(),
                text: text.clone(),
                markup: None,
            },
        );
    }
    for entry in &result.entries {
        for sense in &entry.senses {
            for (key, values) in [
                ("translation", &sense.translations),
                ("definition", &sense.definitions),
                ("examples", &sense.examples),
                ("collocations", &sense.collocations),
                ("word_family", &sense.word_family),
                ("synonyms", &sense.synonyms),
            ] {
                for value in values {
                    add(
                        key,
                        &entry.source.name,
                        ResultRow {
                            headword: entry.headword.clone(),
                            part_of_speech: sense.part_of_speech.clone(),
                            text: value.clone(),
                            // Whole-entry markup only describes a single definition exactly.
                            markup: if key == "definition"
                                && entry.senses.len() == 1
                                && values.len() == 1
                            {
                                entry.definition_markup.clone()
                            } else {
                                None
                            },
                        },
                    );
                }
            }
            for antonym in &sense.antonyms {
                add(
                    "synonyms",
                    &entry.source.name,
                    ResultRow {
                        headword: entry.headword.clone(),
                        part_of_speech: sense.part_of_speech.clone(),
                        text: format!("Opposite: {antonym}"),
                        markup: None,
                    },
                );
            }
        }
    }
    for suggestion in &result.suggestions {
        let kind = match suggestion.kind {
            SuggestionKind::Rewrite => "Rewrite",
            SuggestionKind::Spelling => "Spelling",
            SuggestionKind::Grammar => "Grammar",
            SuggestionKind::Completion => "Completion",
        };
        add(
            "suggestions",
            &suggestion.provider,
            ResultRow {
                headword: String::new(),
                part_of_speech: String::new(),
                text: format!("{kind}: {}", suggestion.replacement),
                markup: None,
            },
        );
    }
    CATEGORIES
        .into_iter()
        .filter_map(|(key, title)| {
            let groups = content.remove(key).unwrap_or_default();
            let rows: Vec<_> = groups
                .iter()
                .flat_map(|group| group.rows.iter().map(|row| row.text.clone()))
                .collect();
            let selected_translation =
                key == "translation" && result.snapshot.source == "selection";
            if rows.is_empty() && !result.writing && !selected_translation {
                return None;
            }
            let body = if rows.is_empty() {
                if selected_translation {
                    if result.status == "Generating…" {
                        "Translating selected text…"
                    } else if !result.status.is_empty() {
                        &result.status
                    } else {
                        "No translation available"
                    }
                } else if key == "suggestions" {
                    if result.status == "Generating…" {
                        "Checking your wording…"
                    } else {
                        "No writing suggestion"
                    }
                } else if result.status == "Generating…" {
                    "Generating details…"
                } else {
                    "No information available for this word"
                }
                .into()
            } else {
                rows.join("\n\n")
            };
            let copy_text = match key {
                "translation" => result
                    .translation
                    .as_ref()
                    .map(|t| t.0.clone())
                    .or_else(|| rows.first().cloned())
                    .unwrap_or_default(),
                "suggestions" => result
                    .suggestions
                    .first()
                    .and_then(writy_core::types::Suggestion::replaced_text)
                    .unwrap_or_default(),
                _ => rows.join("\n"),
            };
            Some(Panel {
                category: key,
                title,
                body,
                groups,
                copy_text,
            })
        })
        .collect()
}

impl Panel {
    fn search_row<'a>(
        &self,
        group: &SourceGroup,
        row: &'a ResultRow,
    ) -> (Cow<'a, str>, Option<&'a str>) {
        if self.category == "definition" && group.source == "Arramooz Alwaseet" {
            (Cow::Owned(compact_arramooz(&row.text)), None)
        } else {
            (Cow::Borrowed(&row.text), row.markup.as_deref())
        }
    }
}

#[derive(Debug, PartialEq)]
enum DisplayText {
    Plain(String),
    Markup(String),
}
impl SourceGroup {
    fn body(&self) -> DisplayText {
        if self.rows.iter().any(|row| row.markup.is_some()) {
            DisplayText::Markup(
                self.rows
                    .iter()
                    .map(|row| {
                        row.markup
                            .clone()
                            .unwrap_or_else(|| gtk::glib::markup_escape_text(&row.text).to_string())
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            )
        } else {
            DisplayText::Plain(
                self.rows
                    .iter()
                    .map(|row| row.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            )
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use writy_core::types::{Confidence, Language, TextSnapshot};
    #[test]
    fn arramooz_details_keep_reading_order_without_empty_lines() {
        assert_eq!(
            compact_arramooz("فعل [لعب]\n\nثلاثي\n\nلازم\nيتصرف في: الماضي، المضارع"),
            "فعل [لعب]\nثلاثي · لازم\nيتصرف في: الماضي، المضارع"
        );
        assert_eq!(compact_arramooz("ثلاثي\nمتعدي"), "ثلاثي · متعدي");
        assert_eq!(
            compact_arramooz("تعريف أول\nتعريف ثان"),
            "تعريف أول\nتعريف ثان"
        );
    }
    #[test]
    fn repeated_arabic_entries_are_deduplicated_without_merging_sources_or_headwords() {
        use writy_core::types::{Attribution, LexicalEntry, Sense};
        let entry = LexicalEntry {
            headword: "لَعِبَ".into(),
            source: Attribution {
                name: "Arramooz Alwaseet".into(),
                ..Default::default()
            },
            senses: vec![Sense {
                definitions: vec!["فعل [لعب]\nثلاثي\nلازم".into()],
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut other_word = entry.clone();
        other_word.headword = "لَعِبَ بالألفاظ".into();
        let mut other_source = entry.clone();
        other_source.source.name = "Another dictionary".into();
        let result = Results {
            writing: false,
            output_bounds: None,
            generation: 1,
            snapshot: TextSnapshot {
                source: "test".into(),
                focus_epoch: 1,
                revision: 1,
                language: Language::Arabic,
                text: "لعب".into(),
                confidence: Confidence::Projected,
            },
            entries: vec![entry.clone(), entry, other_word, other_source],
            suggestions: vec![],
            translation: None,
            status: String::new(),
            caret: None,
            output: None,
        };
        let panels = panels(&result);
        let definition = panels
            .iter()
            .find(|panel| panel.category == "definition")
            .unwrap();
        assert_eq!(definition.groups.len(), 2);
        let row = &definition.groups[0].rows[0];
        assert_eq!(
            definition.search_row(&definition.groups[0], row).0,
            "فعل [لعب]\nثلاثي · لازم"
        );
        assert_eq!(row.text, "فعل [لعب]\nثلاثي\nلازم");
        assert!(definition.copy_text.contains(&row.text));
        assert!(
            matches!(definition.groups[0].body(), DisplayText::Plain(text) if text.contains(&row.text))
        );

        assert_eq!(
            definition.groups[0].rows.len(),
            2,
            "Repeated records disappear, distinct headwords stay"
        );
        assert_eq!(
            definition.groups[1].rows.len(),
            1,
            "Independent sources retain attribution"
        );
    }

    #[test]
    fn mixed_markup_escapes_plain_rows_and_search_compaction_suppresses_markup() {
        let row = |text: &str, markup: Option<&str>| ResultRow {
            headword: String::new(),
            part_of_speech: String::new(),
            text: text.into(),
            markup: markup.map(str::to_owned),
        };
        let mut group = SourceGroup {
            source: "fixture".into(),
            rows: vec![row("bold", Some("<b>bold</b>")), row("x < y & z", None)],
        };
        assert_eq!(
            group.body(),
            DisplayText::Markup("<b>bold</b>\n\nx &lt; y &amp; z".into())
        );
        let panel = Panel {
            category: "definition",
            title: "Definition",
            body: String::new(),
            groups: vec![],
            copy_text: String::new(),
        };
        assert_eq!(
            panel.search_row(&group, &group.rows[0]).1,
            Some("<b>bold</b>")
        );
        group.source = "Arramooz Alwaseet".into();
        assert_eq!(panel.search_row(&group, &group.rows[0]).0, "bold");
        assert_eq!(panel.search_row(&group, &group.rows[0]).1, None);
        assert_eq!(group.rows[0].markup.as_deref(), Some("<b>bold</b>"));
    }

    #[test]
    fn translation_display_and_copy_contain_only_the_translation() {
        let result = Results {
            writing: true,
            output_bounds: None,
            generation: 1,
            snapshot: TextSnapshot {
                source: "test".into(),
                focus_epoch: 1,
                revision: 1,
                language: Language::English,
                text: "hello".into(),
                confidence: Confidence::Projected,
            },
            entries: vec![],
            suggestions: vec![],
            translation: Some(("مرحبا".into(), "model · generated".into())),
            status: String::new(),
            caret: None,
            output: None,
        };
        let mut selected_sentence = result.clone();
        selected_sentence.writing = false;
        selected_sentence.snapshot.source = "selection".into();
        selected_sentence.snapshot.text = "I will attend the meeting tomorrow.".into();
        selected_sentence.translation = None;
        selected_sentence.status = "Generating…".into();
        let pending = panels(&selected_sentence);
        assert_eq!(
            pending.len(),
            1,
            "A selected sentence must open Translation while the model works"
        );
        assert_eq!(pending[0].category, "translation");
        assert!(pending[0].copy_text.is_empty());

        let panels = panels(&Results {
            status: "Generating…".into(),
            ..result
        });
        assert_eq!(panels.len(), 7);
        assert_eq!(panels[0].body, "مرحبا");
        assert_eq!(panels[0].copy_text, "مرحبا");
        assert!(panels[1].copy_text.is_empty());
    }
}

#[derive(Default)]
pub(crate) struct DisplayGeneration {
    displayed: u64,
    pending_clear: Option<u64>,
}
impl DisplayGeneration {
    pub fn shown(&mut self, generation: u64) {
        self.displayed = generation;
    }
    pub fn accept(&mut self, generation: u64, latest: u64) -> bool {
        if generation != latest {
            return false;
        }
        self.shown(generation);
        true
    }
    pub fn end_frame(&mut self, latest: u64) -> bool {
        if self.displayed == latest {
            self.pending_clear = None;
            return false;
        }
        // Give an in-flight Clear one frame before failing closed on a dropped event.
        if self.pending_clear == Some(latest) {
            self.displayed = latest;
            self.pending_clear = None;
            true
        } else {
            self.pending_clear = Some(latest);
            false
        }
    }
}

struct View {
    app: gtk::Application,
    tx: Commands,
    config: Config,
    search: gtk::ApplicationWindow,
    editor: gtk::TextView,
    search_debounce: Rc<RefCell<Option<glib::SourceId>>>,
    results: gtk::Notebook,
    tab_copy_texts: Rc<RefCell<Vec<String>>>,
    status: gtk::Label,
    activity: gtk::Spinner,
    copy: gtk::Button,
    search_copy_text: Rc<RefCell<String>>,
    content: gtk::Stack,
    empty: gtk::Box,
    hud: hud::HudStack,
    generation: DisplayGeneration,
    last_result: Option<Results>,
    layout_monitor: Option<gdk::Monitor>,
    layout_bounds: Option<(i32, i32, i32, i32)>,
}
impl View {
    fn new(app: &gtk::Application, tx: Commands, config: Config) -> Rc<RefCell<Self>> {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(CSS);
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        let search = gtk::ApplicationWindow::builder()
            .application(app)
            .title("Writy — English ↔ العربية")
            .default_width(1000)
            .default_height(760)
            .build();
        search.add_css_class("writy-main");
        let root = gtk::Box::new(gtk::Orientation::Vertical, 14);
        root.set_margin_top(14);
        root.set_margin_bottom(29);
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        header.add_css_class("writy-search-header");
        header.set_margin_bottom(5);
        let name = label("Writy", "writy-header");
        name.set_wrap(false);
        name.set_hexpand(true);
        header.append(&name);
        let languages = label(
            "English ↔ العربية  ·  Read, translate, write naturally",
            "writy-meta",
        );
        languages.set_direction(gtk::TextDirection::Ltr);
        languages.set_hexpand(true);
        let header_details = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        header_details.set_hexpand(false);
        header_details.append(&languages);
        let toolbar = gtk::HeaderBar::new();
        toolbar.add_css_class("writy-toolbar");
        toolbar.set_title_widget(Some(&gtk::Box::new(gtk::Orientation::Horizontal, 0)));
        let settings = gtk::Button::from_icon_name("emblem-system-symbolic");
        settings.set_tooltip_text(Some("Settings"));
        settings.update_property(&[gtk::accessible::Property::Label("Settings")]);
        let stx = tx.clone();
        settings.connect_clicked(move |_| send(&stx, Command::Settings));
        header_details.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        header_details.append(&settings);
        header.append(&header_details);
        search.set_titlebar(Some(&toolbar));
        root.append(&header);
        let editor = gtk::TextView::new();
        editor.set_wrap_mode(gtk::WrapMode::WordChar);
        editor.add_css_class("writy-query");
        editor.set_top_margin(8);
        editor.set_bottom_margin(8);
        let input_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .min_content_height(100)
            .max_content_height(180)
            .child(&editor)
            .build();
        input_scroll.add_css_class("writy-input");
        editor.update_property(&[gtk::accessible::Property::Label("Word or sentence")]);
        let input = gtk::Box::new(gtk::Orientation::Vertical, 7);
        input.append(&label("Word or sentence", "writy-title"));
        input.append(&input_scroll);
        root.append(&input);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        actions.add_css_class("writy-search-actions");
        for (name, translate) in [("Look up or improve", false), ("Translate", true)] {
            let b = components::action_button(name);
            b.set_hexpand(true);
            if !translate {
                b.add_css_class("suggested-action");
            }
            b.set_sensitive(false);
            let action = b.downgrade();
            editor.buffer().connect_changed(move |buffer| {
                if let Some(action) = action.upgrade() {
                    action.set_sensitive(
                        !buffer
                            .text(&buffer.start_iter(), &buffer.end_iter(), false)
                            .trim()
                            .is_empty(),
                    );
                }
            });
            let buffer = editor.buffer();
            let tx = tx.clone();
            b.connect_clicked(move |_| {
                let text = buffer
                    .text(&buffer.start_iter(), &buffer.end_iter(), false)
                    .to_string();
                send(&tx, Command::Query { text, translate });
            });
            actions.append(&b);
        }
        let search_copy_text = Rc::new(RefCell::new(String::new()));
        let copy = copy_button(search_copy_text.clone(), "Copy result");
        copy.set_halign(gtk::Align::Fill);
        copy.set_sensitive(false);
        actions.append(&copy);
        let controls = gtk::Box::new(gtk::Orientation::Vertical, 5);
        controls.set_margin_bottom(10);
        controls.append(&actions);
        root.append(&controls);
        let status = label(
            "Type a word or sentence. Local dictionaries work without a network.",
            "writy-status",
        );
        let copy_status = status.clone();
        copy.connect_clicked(move |_| copy_status.set_text("Copied to clipboard"));
        let status_row = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        status_row.add_css_class("writy-search-status");
        let activity = gtk::Spinner::new();
        activity.set_visible(false);
        status_row.append(&activity);
        status_row.append(&status);
        controls.append(&status_row);
        let weak_status_row = status_row.downgrade();
        status.connect_label_notify(move |status| {
            if let Some(row) = weak_status_row.upgrade() {
                row.set_visible(!status.text().is_empty());
            }
        });
        let results = gtk::Notebook::new();
        components::enable_wheel_tab_switching(&results);
        results.set_scrollable(true);
        results.set_vexpand(true);
        results.set_size_request(-1, 220);
        results.add_css_class("writy-result-tabs");
        let tab_copy_texts = Rc::new(RefCell::new(Vec::<String>::new()));
        let texts = tab_copy_texts.clone();
        let active_text = search_copy_text.clone();
        let active_copy = copy.clone();
        results.connect_switch_page(move |_, _, page| {
            let text = texts
                .borrow()
                .get(page as usize)
                .cloned()
                .unwrap_or_default();
            active_copy.set_sensitive(!text.is_empty());
            *active_text.borrow_mut() = text;
        });
        let content = gtk::Stack::new();
        content.set_vexpand(true);
        // No moving surfaces: changing results must not distract from typing.
        content.set_transition_type(gtk::StackTransitionType::None);
        let empty = gtk::Box::new(gtk::Orientation::Vertical, 0);
        empty.append(&empty_state(
            "Find the right words",
            "Look up a word, improve a sentence, or translate between English and Arabic.",
        ));
        content.add_named(&empty, Some("empty"));
        content.add_named(&results, Some("results"));
        content.set_visible_child_name("empty");
        root.append(&content);
        root.append(&label(
            "Enter copies the result · Shift+Enter adds a line · Esc closes",
            "writy-meta",
        ));
        let column = components::ReadingColumn::new(root.upcast_ref(), &actions, &header);
        column.set_vexpand(true);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&column)
            .build();
        scroll.set_overlay_scrolling(false);
        search.set_child(Some(&scroll));
        let txclose = tx.clone();
        search.connect_close_request(move |w| {
            w.set_visible(false);
            send(&txclose, Command::Dismiss);
            glib::Propagation::Stop
        });
        let key = gtk::EventControllerKey::new();
        let keytx = tx.clone();
        let weak = search.downgrade();
        let key_editor = editor.clone();
        let key_copy = copy.clone();
        key.connect_key_pressed(move |_, key, _, mods| {
            if key == gdk::Key::Escape {
                if let Some(w) = weak.upgrade() {
                    w.set_visible(false);
                }
                send(&keytx, Command::Dismiss);
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::Return
                && key_editor.has_focus()
                && !mods.contains(gdk::ModifierType::SHIFT_MASK)
            {
                if key_copy.is_sensitive() {
                    key_copy.emit_clicked();
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        search.add_controller(key);
        let debounce = Rc::new(RefCell::new(None::<glib::SourceId>));
        let search_debounce = debounce.clone();
        let qtx = tx.clone();
        editor.buffer().connect_changed(move |buffer| {
            if let Some(id) = debounce.borrow_mut().take() {
                id.remove();
            }
            let text = buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), false)
                .to_string();
            let tx = qtx.clone();
            let pending = debounce.clone();
            *debounce.borrow_mut() = Some(glib::timeout_add_local_once(
                Duration::from_millis(300),
                move || {
                    pending.borrow_mut().take();
                    send(
                        &tx,
                        Command::Query {
                            text,
                            translate: false,
                        },
                    );
                },
            ));
        });
        let hud = hud::HudStack::new(app, &tx);
        Rc::new(RefCell::new(Self {
            app: app.clone(),
            tx,
            config,
            search,
            editor,
            search_debounce,
            results,
            tab_copy_texts,
            status,
            activity,
            copy,
            search_copy_text,
            content,
            empty,
            hud,
            generation: DisplayGeneration::default(),
            last_result: None,
            layout_monitor: None,
            layout_bounds: None,
        }))
    }
    fn open_search(&self, fallback: Option<String>) {
        let editor = self.editor.clone();
        let debounce = self.search_debounce.clone();
        let search = self.search.clone();
        let tx = self.tx.clone();
        self.hud.window.set_visible(false);
        glib::MainContext::default().spawn_local(async move {
            let display = gtk::prelude::WidgetExt::display(&search);
            let mut query = fallback;
            // CLI-captured text survives a cold start; GTK clipboard access may require focus.
            if query.is_none() {
                // Read PRIMARY before presenting the window changes keyboard focus.
                for clipboard in [display.primary_clipboard(), display.clipboard()] {
                    if let Ok(Ok(Some(text))) = glib::future_with_timeout(
                        Duration::from_secs(1),
                        clipboard.read_text_future(),
                    )
                    .await
                        && !text.trim().is_empty()
                    {
                        query = Some(text.to_string());
                        break;
                    }
                }
            }
            if let Some(query) = query {
                editor.buffer().set_text(&query);
                // The editor change schedules a lookup; it must not cancel this translation.
                if let Some(pending) = debounce.borrow_mut().take() {
                    pending.remove();
                }
                send(
                    &tx,
                    Command::Query {
                        text: query,
                        translate: true,
                    },
                );
            } else {
                let buffer = editor.buffer();
                let text = buffer
                    .text(&buffer.start_iter(), &buffer.end_iter(), false)
                    .to_string();
                if !text.trim().is_empty() {
                    send(
                        &tx,
                        Command::Query {
                            text,
                            translate: false,
                        },
                    );
                }
            }
            search.present();
            editor.grab_focus();
        });
    }
    fn clear(&mut self) {
        self.last_result = None;
        self.copy.set_sensitive(false);
        self.search_copy_text.borrow_mut().clear();
        self.activity.set_spinning(false);
        self.activity.set_visible(false);
        self.content.set_visible_child_name("empty");
        while let Some(child) = self.empty.first_child() {
            self.empty.remove(&child);
        }
        let buffer = self.editor.buffer();
        let has_query = !buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .trim()
            .is_empty();
        self.empty.append(&if has_query {
            empty_state(
                "Ready to look up",
                "Run lookup or translation for the text above.",
            )
        } else {
            empty_state(
                "Find the right words",
                "Look up a word, improve a sentence, or translate between English and Arabic.",
            )
        });
        self.hud.window.set_visible(false);
        self.tab_copy_texts.borrow_mut().clear();
        while self.results.n_pages() > 0 {
            self.results.remove_page(Some(0));
        }
    }
    fn show(&mut self, result: Results) {
        self.generation.shown(result.generation);
        let selected_tab = self
            .results
            .current_page()
            .and_then(|page| self.results.nth_page(Some(page)))
            .and_then(|page| self.results.tab_label_text(&page));
        if let Ok(config) = Config::load() {
            self.config = config;
        }
        self.tab_copy_texts.borrow_mut().clear();
        while self.results.n_pages() > 0 {
            self.results.remove_page(Some(0));
        }
        self.status.set_text(&result.status);
        let generating = result.status == "Generating…";
        self.activity.set_visible(generating);
        self.activity.set_spinning(generating);
        self.editor.set_direction(
            if result.snapshot.language == writy_core::types::Language::Arabic {
                gtk::TextDirection::Rtl
            } else {
                gtk::TextDirection::Ltr
            },
        );
        let panels = panels(&result);
        self.content.set_visible_child_name(if panels.is_empty() {
            "empty"
        } else {
            "results"
        });
        // Replace the welcome message only when a completed lookup has no content.
        if panels.is_empty() {
            while let Some(child) = self.empty.first_child() {
                self.empty.remove(&child);
            }
            self.empty.append(&if generating {
                empty_state(
                    "Finding the right words…",
                    "You can keep editing while Writy works.",
                )
            } else {
                empty_state(
                    "No results found",
                    "Try another word or download a dictionary in Settings.",
                )
            });
        }
        let display = gdk::Display::default();
        let monitors: Vec<gdk::Monitor> = display
            .as_ref()
            .map(|d| {
                d.monitors()
                    .iter::<gdk::Monitor>()
                    .filter_map(Result::ok)
                    .collect()
            })
            .unwrap_or_default();
        let monitor = result
            .output
            .as_ref()
            .and_then(|name| {
                monitors
                    .iter()
                    .find(|m| m.connector().as_deref() == Some(name.as_str()))
                    .cloned()
            })
            .or_else(|| {
                result
                    .caret
                    .and_then(|(x, y, _, _)| {
                        monitors
                            .iter()
                            .find(|m| {
                                let b = m.geometry();
                                x >= b.x()
                                    && x < b.x() + b.width()
                                    && y >= b.y()
                                    && y < b.y() + b.height()
                            })
                            .cloned()
                    })
                    .or_else(|| monitors.first().cloned())
            });
        let bounds = monitor
            .as_ref()
            .map(gdk::Monitor::geometry)
            .unwrap_or(gdk::Rectangle::new(0, 0, 1920, 1080));
        self.last_result = Some(result.clone());
        self.layout_monitor = monitor.clone();
        self.layout_bounds = Some((bounds.x(), bounds.y(), bounds.width(), bounds.height()));
        let bounds = result
            .output_bounds
            .filter(|(_, _, width, height)| {
                *width > 0 && *height > 0 && *width <= 65536 && *height <= 65536
            })
            .map(|(x, y, width, height)| gdk::Rectangle::new(x, y, width, height))
            .unwrap_or(bounds);
        if self.search.is_visible() {
            self.hud.window.set_visible(false);
        } else {
            self.hud
                .update(&result, &panels, &self.config, monitor.as_ref(), bounds);
        }
        let mut selected_page = 0;
        for (index, panel) in panels.iter().enumerate() {
            self.tab_copy_texts
                .borrow_mut()
                .push(panel.copy_text.clone());
            let page = gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Never)
                .child(&result_card(panel))
                .build();
            page.set_overlay_scrolling(true);
            let tab = components::tab_label(panel.title);
            self.results.append_page(&page, Some(&tab));
            if selected_tab.as_deref() == Some(panel.title) {
                selected_page = index as u32;
            }
        }
        let details = gtk::Box::new(gtk::Orientation::Vertical, 7);
        details.add_css_class("writy-result-section");
        let title = label("Sources", "writy-section-title");
        details.append(&title);
        for (index, entry) in result.entries.iter().enumerate() {
            if index > 0 {
                details.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            }
            if !entry.pronunciation.is_empty() {
                details.append(&label(&entry.pronunciation, "writy-meta"));
            }
            for path in entry.audio.iter().take(2) {
                let b = components::action_button("Play pronunciation");
                let path = path.clone();
                b.connect_clicked(move |_| {
                    if let Ok(mut child) = std::process::Command::new("mpv")
                        .args(["--no-video", "--really-quiet", "--"])
                        .arg(&path)
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                    {
                        std::thread::spawn(move || {
                            let _ = child.wait();
                        });
                    }
                });
                details.append(&b);
            }
            for path in entry.images.iter().take(2) {
                if let Some((_, width, height)) = gtk::gdk_pixbuf::Pixbuf::file_info(path)
                    && width > 0
                    && height > 0
                    && i64::from(width) * i64::from(height) <= 16_777_216
                    && let Ok(pixbuf) =
                        gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, 420, 120, true)
                {
                    let picture = gtk::Picture::for_paintable(&gdk::Texture::for_pixbuf(&pixbuf));
                    picture.set_can_shrink(true);
                    details.append(&picture);
                }
            }
            let attribution = label(
                &format!(
                    "{} · {} · {}",
                    entry.source.name, entry.source.license, entry.source.url
                ),
                "writy-meta",
            );
            attribution.set_selectable(true);
            details.append(&attribution);
        }
        if !result.entries.is_empty() {
            self.tab_copy_texts.borrow_mut().push(String::new());
            let page = gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Never)
                .child(&details)
                .build();
            self.results
                .append_page(&page, Some(&components::tab_label("Sources")));
            if selected_tab.as_deref() == Some("Sources") {
                selected_page = self.results.n_pages() - 1;
            }
        }
        self.results.set_current_page(Some(selected_page));
        let text = self
            .tab_copy_texts
            .borrow()
            .get(selected_page as usize)
            .cloned()
            .unwrap_or_default();
        self.copy.set_sensitive(!text.is_empty());
        *self.search_copy_text.borrow_mut() = text;
    }
}
pub fn run(config: Config) -> glib::ExitCode {
    let app = gtk::Application::builder()
        .application_id("io.github.lodaq.writy")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    let _hold = app.hold();
    app.connect_activate(move |app| {
        let (tx, rx) = std::sync::mpsc::sync_channel(128);
        let generation = Arc::new(AtomicU64::new(0));
        let commands = crate::backend::start(config.clone(), tx, generation.clone());
        let view = View::new(app, commands, config.clone());
        attach(view, rx, generation);
    });
    app.run_with_args::<&str>(&[])
}
fn attach(view: Rc<RefCell<View>>, rx: Receiver<UiEvent>, generation: Arc<AtomicU64>) {
    glib::timeout_add_local(Duration::from_millis(33), move || {
        let mut view = view.borrow_mut();
        if let Some(monitor) = &view.layout_monitor {
            let b = monitor.geometry();
            if view.layout_bounds != Some((b.x(), b.y(), b.width(), b.height()))
                && let Some(result) = view.last_result.clone()
            {
                view.show(result);
            }
        }
        for _ in 0..128 {
            let Ok(event) = rx.try_recv() else {
                break;
            };
            match event {
                UiEvent::Clear {
                    generation: cleared,
                    preserve_huds,
                } => {
                    if view
                        .generation
                        .accept(cleared, generation.load(Ordering::SeqCst))
                    {
                        if preserve_huds {
                            // These are read-only reference panels; stale correction
                            // anchors are discarded in the backend independently.
                            view.last_result = None;
                        } else {
                            view.clear();
                        }
                    }
                }
                UiEvent::Screenshot(active) => {
                    if layer_shell_supported() {
                        // Let the fullscreen capture/annotation surface receive input.
                        view.hud
                            .window
                            .set_layer(if active { Layer::Top } else { Layer::Overlay });
                    }
                }
                UiEvent::Search(query) => view.open_search(query),
                UiEvent::Dismiss => {
                    for window in view.app.windows() {
                        window.set_visible(false);
                    }
                }
                UiEvent::Settings => view.settings(),
                UiEvent::Results(result) => {
                    if view
                        .generation
                        .accept(result.generation, generation.load(Ordering::SeqCst))
                    {
                        view.show(*result);
                    }
                }
                UiEvent::Notice(notice) => view.status.set_text(&notice),
                UiEvent::Copy(text) => {
                    clipboard(&text);
                    view.status.set_text("Copied to clipboard");
                }
            }
        }
        if view.generation.end_frame(generation.load(Ordering::SeqCst)) {
            view.clear();
        }
        glib::ControlFlow::Continue
    });
}

#[cfg(test)]
#[path = "ui_tests.rs"]
mod visual_tests;

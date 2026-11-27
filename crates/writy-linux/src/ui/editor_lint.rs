//! Inline spelling, grammar, and improvement highlights with click-to-fix popovers.
use super::*;
use std::cell::Cell;
use std::sync::{
    Condvar, Mutex,
    mpsc::{self, Receiver, TryRecvError},
};
use writy_core::{
    config::HarperConfig,
    linter::{EnglishChecker, LintFinding},
    types::{Suggestion, SuggestionKind},
};

const TAG_SPELLING: &str = "writy-lint-spelling";
const TAG_GRAMMAR: &str = "writy-lint-grammar";
const TAG_IMPROVEMENT: &str = "writy-lint-improvement";
const LOCAL_DEBOUNCE_MS: u64 = 75;

#[derive(Clone)]
pub(super) struct EditorFinding {
    pub start: usize,
    pub end: usize,
    pub vis_start: usize,
    pub vis_end: usize,
    pub original: String,
    pub kind: SuggestionKind,
    pub category: String,
    pub rule: String,
    pub message: String,
    pub replacements: Vec<String>,
    pub provider: String,
    pub result_suggestion: Option<Suggestion>,
    pub result_generation: u64,
    pub ignore_hash: Option<u64>,
}

impl EditorFinding {
    fn tag_name(&self) -> &'static str {
        match self.kind {
            SuggestionKind::Spelling => TAG_SPELLING,
            SuggestionKind::Grammar => TAG_GRAMMAR,
            SuggestionKind::Rewrite => TAG_IMPROVEMENT,
        }
    }

    fn accent_class(&self) -> &'static str {
        match self.kind {
            SuggestionKind::Spelling => "writy-lint-spelling",
            SuggestionKind::Grammar => "writy-lint-grammar",
            SuggestionKind::Rewrite => "writy-lint-improvement",
        }
    }

    fn dismiss_key(&self) -> DismissKey {
        DismissKey {
            original: self.original.clone(),
            start: self.start,
            end: self.end,
            rule: self.rule.clone(),
            category: self.category.clone(),
            replacements: self.replacements.clone(),
        }
    }

    fn can_add_to_dictionary(&self) -> bool {
        self.rule == "SpellCheck"
            && self.kind == SuggestionKind::Spelling
            && writy_core::types::is_single_word(&self.original)
    }
}

#[derive(Clone, PartialEq, Eq)]
struct DismissKey {
    original: String,
    start: usize,
    end: usize,
    rule: String,
    category: String,
    replacements: Vec<String>,
}

#[derive(Default)]
struct WorkerSlot {
    pending: Option<(u64, HarperConfig, String)>,
    closed: bool,
}

struct WorkerState {
    slot: Arc<(Mutex<WorkerSlot>, Condvar)>,
    rx: Receiver<(u64, String, Vec<LintFinding>)>,
    polling: bool,
}

impl Drop for WorkerState {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.slot.0.lock() {
            guard.closed = true;
            self.slot.1.notify_one();
        }
    }
}

struct Inner {
    editor: gtk::TextView,
    tx: Commands,
    harper: HarperConfig,
    local_findings: Vec<EditorFinding>,
    ai_findings: Vec<EditorFinding>,
    active_findings: Vec<EditorFinding>,
    dismissed: Vec<DismissKey>,
    checked_text: String,
    debounce: Option<glib::SourceId>,
    seq: u64,
    worker: Option<WorkerState>,
    popover: gtk::Popover,
    open_settings: Rc<Cell<bool>>,
}

#[derive(Clone)]
pub(super) struct EditorLintController {
    inner: Rc<RefCell<Inner>>,
}

impl EditorLintController {
    pub fn new(
        editor: &gtk::TextView,
        tx: &Commands,
        harper: &HarperConfig,
        open_settings: Rc<Cell<bool>>,
    ) -> Self {
        ensure_tags(&editor.buffer());

        let popover = gtk::Popover::new();
        popover.set_parent(editor);
        popover.set_autohide(true);
        popover.set_has_arrow(true);
        popover.set_position(gtk::PositionType::Bottom);
        popover.add_css_class("writy-lint-popover");

        let inner = Rc::new(RefCell::new(Inner {
            editor: editor.clone(),
            tx: tx.clone(),
            harper: harper.clone(),
            local_findings: Vec::new(),
            ai_findings: Vec::new(),
            active_findings: Vec::new(),
            dismissed: Vec::new(),
            checked_text: String::new(),
            debounce: None,
            seq: 0,
            worker: None,
            popover: popover.clone(),
            open_settings,
        }));

        // Close popover cleanly on Escape or focus loss without stealing editor bindings.
        let controller = Self {
            inner: inner.clone(),
        };

        // Re-check locally while typing.
        let changed_ctrl = controller.clone();
        editor.buffer().connect_changed(move |_| {
            changed_ctrl.on_buffer_changed();
        });

        // Open suggestion popover on click over a highlighted span.
        // Capture phase ensures GtkTextView's internal click gesture cannot swallow release events.
        let click = gtk::GestureClick::new();
        click.set_button(1);
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        let click_ctrl = controller.clone();
        click.connect_released(move |_, n_press, x, y| {
            if n_press != 1 {
                return;
            }
            let ctrl = click_ctrl.clone();
            glib::idle_add_local_once(move || {
                ctrl.handle_click(x, y);
            });
        });
        editor.add_controller(click);

        // Keyboard shortcut: Ctrl+. or Menu opens suggestions at the cursor.
        let key = gtk::EventControllerKey::new();
        key.set_propagation_phase(gtk::PropagationPhase::Capture);
        let key_ctrl = controller.clone();
        key.connect_key_pressed(move |_, keyval, _, modifiers| {
            let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
            if ((ctrl && keyval == gdk::Key::period) || keyval == gdk::Key::Menu)
                && key_ctrl.open_at_cursor()
            {
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        editor.add_controller(key);

        controller
    }

    pub fn update_harper_config(&self, config: &HarperConfig) {
        let changed = {
            let mut inner = self.inner.borrow_mut();
            if inner.harper == *config {
                false
            } else {
                inner.harper = config.clone();
                true
            }
        };
        if changed {
            self.schedule_local_check(0);
        }
    }

    pub fn merge_results(&self, result: &Results) {
        let mut inner = self.inner.borrow_mut();
        let plain = plain_buffer_text(&inner.editor.buffer());
        if plain.trim() != result.snapshot.text {
            return;
        }
        let leading = plain.chars().count() - plain.trim_start().chars().count();
        let plain_chars: Vec<char> = plain.chars().collect();
        let plain_len = plain_chars.len();

        let mut ai = Vec::new();
        for suggestion in &result.suggestions {
            // Harper suggestions are produced locally with full multi-choice alternatives and rules.
            // Single-word dictionary synonyms are shown in the results tabs without underlining valid words.
            if suggestion.provider == "Harper · local"
                || suggestion.description == "Synonym; choose one that fits your meaning"
            {
                continue;
            }
            let start = leading + suggestion.start;
            let end = leading + suggestion.end;
            if start > end || end > plain_len {
                continue;
            }
            let (vis_start, vis_end) = visual_span(start, end, plain_len);
            let original: String = plain_chars[start..end].iter().collect();
            let category = match suggestion.kind {
                SuggestionKind::Spelling => "Spelling",
                SuggestionKind::Grammar => "Grammar",
                SuggestionKind::Rewrite => "Improvement",
            }
            .to_owned();
            ai.push(EditorFinding {
                start,
                end,
                vis_start,
                vis_end,
                original,
                kind: suggestion.kind,
                category,
                rule: String::new(),
                message: suggestion.description.clone(),
                replacements: vec![suggestion.replacement.clone()],
                provider: suggestion.provider.clone(),
                result_suggestion: Some(suggestion.clone()),
                result_generation: result.generation,
                ignore_hash: suggestion.ignore_hash,
            });
        }
        inner.ai_findings = ai;
        refresh_tags(&mut inner, &plain);
    }

    /// Synchronous check used by tests and immediate post-fix updates.
    pub fn check_now(&self) {
        let (harper, plain) = {
            let mut inner = self.inner.borrow_mut();
            if let Some(id) = inner.debounce.take() {
                id.remove();
            }
            inner.seq += 1;
            let plain = plain_buffer_text(&inner.editor.buffer());
            (inner.harper.clone(), plain)
        };
        let mut checker = EnglishChecker::from_config(&harper);
        let findings = checker.check_findings(&plain);
        let mut inner = self.inner.borrow_mut();
        let current = plain_buffer_text(&inner.editor.buffer());
        if current == plain {
            inner.checked_text = plain.clone();
            inner.local_findings = convert_findings(&plain, findings);
            refresh_tags(&mut inner, &plain);
        }
    }

    #[cfg(test)]
    pub(super) fn active_findings_count(&self) -> usize {
        self.inner.borrow().active_findings.len()
    }

    #[cfg(test)]
    pub(super) fn click_at_char_offset(&self, offset: usize) -> bool {
        self.open_popover_for_offset(offset, false)
    }

    #[cfg(test)]
    pub(super) fn popover(&self) -> gtk::Popover {
        self.inner.borrow().popover.clone()
    }

    fn on_buffer_changed(&self) {
        let popover = {
            let mut inner = self.inner.borrow_mut();
            let popover = inner.popover.clone();
            let plain = plain_buffer_text(&inner.editor.buffer());
            if plain.trim().is_empty() {
                inner.checked_text.clear();
                inner.local_findings.clear();
                inner.ai_findings.clear();
                inner.active_findings.clear();
                inner.dismissed.clear();
                clear_all_tags(&inner.editor.buffer());
                if let Some(id) = inner.debounce.take() {
                    id.remove();
                }
                drop(inner);
                popover.popdown();
                return;
            }
            if inner.checked_text != plain {
                inner.ai_findings.retain(|f| {
                    slice_chars(&plain, f.start, f.end).is_some_and(|s| s == f.original)
                });
                inner.local_findings.retain(|f| {
                    slice_chars(&plain, f.start, f.end).is_some_and(|s| s == f.original)
                });
                refresh_tags(&mut inner, &plain);
            }
            popover
        };
        popover.popdown();
        self.schedule_local_check(LOCAL_DEBOUNCE_MS);
    }

    fn schedule_local_check(&self, delay_ms: u64) {
        let mut inner = self.inner.borrow_mut();
        if let Some(id) = inner.debounce.take() {
            id.remove();
        }
        let plain = plain_buffer_text(&inner.editor.buffer());
        if plain.trim().is_empty() || !inner.harper.enabled {
            inner.local_findings.clear();
            refresh_tags(&mut inner, &plain);
            return;
        }
        let weak = Rc::downgrade(&self.inner);
        inner.debounce = Some(glib::timeout_add_local_once(
            Duration::from_millis(delay_ms),
            move || {
                let Some(inner_rc) = weak.upgrade() else {
                    return;
                };
                let ctrl = Self { inner: inner_rc };
                ctrl.dispatch_worker_check();
            },
        ));
    }

    fn dispatch_worker_check(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.debounce = None;
        let plain = plain_buffer_text(&inner.editor.buffer());
        if plain.trim().is_empty() || !inner.harper.enabled {
            inner.local_findings.clear();
            refresh_tags(&mut inner, &plain);
            return;
        }
        inner.seq += 1;
        let seq = inner.seq;
        let harper = inner.harper.clone();

        if inner.worker.is_none() {
            inner.worker = Some(spawn_worker());
        }
        if let Some(worker) = &inner.worker
            && let Ok(mut slot) = worker.slot.0.lock()
        {
            slot.pending = Some((seq, harper, plain));
            worker.slot.1.notify_one();
        }
        let should_poll = inner.worker.as_ref().is_some_and(|w| !w.polling);
        if should_poll {
            if let Some(worker) = &mut inner.worker {
                worker.polling = true;
            }
            let weak = Rc::downgrade(&self.inner);
            glib::timeout_add_local(Duration::from_millis(16), move || {
                let Some(inner_rc) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let mut inner = inner_rc.borrow_mut();
                let mut latest = None;
                let mut disconnected = false;
                if let Some(worker) = &inner.worker {
                    loop {
                        match worker.rx.try_recv() {
                            Ok(msg) => latest = Some(msg),
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                disconnected = true;
                                break;
                            }
                        }
                    }
                }
                if disconnected {
                    inner.worker = None;
                    return glib::ControlFlow::Break;
                }
                if let Some((result_seq, checked_text, findings)) = latest {
                    let current = plain_buffer_text(&inner.editor.buffer());
                    if current == checked_text {
                        inner.checked_text = checked_text.clone();
                        inner.local_findings = convert_findings(&checked_text, findings);
                        refresh_tags(&mut inner, &current);
                    }
                    if result_seq >= inner.seq {
                        if let Some(worker) = &mut inner.worker {
                            worker.polling = false;
                        }
                        return glib::ControlFlow::Break;
                    }
                }
                glib::ControlFlow::Continue
            });
        }
    }

    fn handle_click(&self, x: f64, y: f64) {
        let (buffer, iter) = {
            let inner = self.inner.borrow();
            let buffer = inner.editor.buffer();
            if buffer.has_selection() {
                return;
            }
            let (bx, by) = inner.editor.window_to_buffer_coords(
                gtk::TextWindowType::Widget,
                x.round() as i32,
                y.round() as i32,
            );
            let Some(iter) = inner.editor.iter_at_location(bx, by) else {
                return;
            };
            (buffer, iter)
        };
        let offset = plain_offset_at_iter(&buffer, &iter);
        self.open_popover_for_offset(offset, false);
    }

    fn open_at_cursor(&self) -> bool {
        let (buffer, offset) = {
            let inner = self.inner.borrow();
            let buffer = inner.editor.buffer();
            let iter = buffer.iter_at_offset(buffer.cursor_position());
            let offset = plain_offset_at_iter(&buffer, &iter);
            (buffer, offset)
        };
        let _ = buffer;
        self.open_popover_for_offset(offset, true)
    }

    fn open_popover_for_offset(&self, offset: usize, allow_trailing_edge: bool) -> bool {
        let matching: Vec<EditorFinding> = {
            let inner = self.inner.borrow();
            inner
                .active_findings
                .iter()
                .filter(|f| {
                    (offset >= f.vis_start && offset < f.vis_end)
                        || (allow_trailing_edge && offset == f.vis_end)
                })
                .cloned()
                .collect()
        };
        if matching.is_empty() {
            return false;
        }

        let (editor, popover) = {
            let inner = self.inner.borrow();
            (inner.editor.clone(), inner.popover.clone())
        };
        let first = &matching[0];
        let buffer = editor.buffer();
        let begin = buffer_iter_at_plain_offset(&buffer, first.vis_start);
        let end = buffer_iter_at_plain_offset(&buffer, first.vis_end);
        let begin_rect = editor.iter_location(&begin);
        let end_rect = editor.iter_location(&end);
        let (wx, wy) = editor.buffer_to_window_coords(
            gtk::TextWindowType::Widget,
            begin_rect.x(),
            begin_rect.y(),
        );
        let width = if end_rect.y() == begin_rect.y() && end_rect.x() > begin_rect.x() {
            end_rect.x() - begin_rect.x()
        } else {
            begin_rect.width().max(24)
        };
        popover.set_pointing_to(Some(&gdk::Rectangle::new(
            wx,
            wy,
            width.max(12),
            begin_rect.height().max(18),
        )));

        let content = self.build_popover_content(matching, 0);
        popover.set_child(Some(&content));
        popover.popup();
        true
    }

    fn build_popover_content(&self, findings: Vec<EditorFinding>, index: usize) -> gtk::Box {
        let finding = &findings[index.min(findings.len() - 1)];
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        root.add_css_class("writy-lint-card-row");

        // Left pane: quick-apply replacements + settings shortcut.
        let left = gtk::Box::new(gtk::Orientation::Vertical, 4);
        left.add_css_class("writy-lint-choices");

        if !finding.replacements.is_empty() {
            for replacement in finding.replacements.iter().take(6) {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                let bar = gtk::Box::new(gtk::Orientation::Vertical, 0);
                bar.add_css_class("writy-lint-bar");
                bar.add_css_class(finding.accent_class());
                row.append(&bar);

                let text = if replacement.is_empty() {
                    "Remove"
                } else {
                    replacement.as_str()
                };
                let name = gtk::Label::new(Some(text));
                name.set_xalign(0.0);
                name.set_hexpand(true);
                name.set_wrap(true);
                name.set_max_width_chars(32);
                name.add_css_class("writy-lint-choice-label");
                if replacement.is_empty() {
                    name.add_css_class("writy-meta");
                }
                row.append(&name);

                let arrow = gtk::Image::from_icon_name("go-next-symbolic");
                arrow.add_css_class("writy-lint-arrow");
                row.append(&arrow);

                let button = gtk::Button::new();
                button.add_css_class("writy-lint-choice");
                button.set_child(Some(&row));

                let ctrl = self.clone();
                let start = finding.start;
                let end = finding.end;
                let original = finding.original.clone();
                let edit = replacement.clone();
                button.connect_clicked(move |_| {
                    ctrl.apply_replacement(start, end, &original, &edit);
                });
                left.append(&button);
            }
        }

        let settings_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let gear = gtk::Image::from_icon_name("emblem-system-symbolic");
        let settings_label = gtk::Label::new(Some("Harper settings"));
        settings_label.set_xalign(0.0);
        settings_row.append(&gear);
        settings_row.append(&settings_label);
        let settings_btn = gtk::Button::new();
        settings_btn.add_css_class("writy-lint-settings-btn");
        settings_btn.set_child(Some(&settings_row));
        let ctrl = self.clone();
        settings_btn.connect_clicked(move |_| {
            let (popover, tx, open_settings) = {
                let inner = ctrl.inner.borrow();
                (
                    inner.popover.clone(),
                    inner.tx.clone(),
                    inner.open_settings.clone(),
                )
            };
            popover.popdown();
            open_settings.set(true);
            send(&tx, Command::Settings);
        });
        left.append(&settings_btn);
        root.append(&left);

        // Right pane: category badge, explanation, dismiss, dictionary, disable rule.
        let right = gtk::Box::new(gtk::Orientation::Vertical, 6);
        right.add_css_class("writy-lint-detail");

        let top_bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        top_bar.add_css_class("writy-lint-top-bar");
        top_bar.add_css_class(finding.accent_class());
        right.append(&top_bar);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let title = gtk::Label::new(Some(&finding.category));
        title.add_css_class("writy-lint-category");
        title.set_xalign(0.0);
        title.set_hexpand(true);
        header.append(&title);

        if findings.len() > 1 {
            let switcher = gtk::Box::new(gtk::Orientation::Horizontal, 2);
            let prev = gtk::Button::from_icon_name("go-previous-symbolic");
            prev.add_css_class("flat");
            prev.set_tooltip_text(Some("Previous issue"));
            prev.set_sensitive(index > 0);
            let ctrl_prev = self.clone();
            let list_prev = findings.clone();
            prev.connect_clicked(move |_| {
                let next_widget = ctrl_prev.build_popover_content(list_prev.clone(), index - 1);
                ctrl_prev
                    .inner
                    .borrow()
                    .popover
                    .set_child(Some(&next_widget));
            });
            let count = gtk::Label::new(Some(&format!("{}/{}", index + 1, findings.len())));
            count.add_css_class("writy-meta");
            let next = gtk::Button::from_icon_name("go-next-symbolic");
            next.add_css_class("flat");
            next.set_tooltip_text(Some("Next issue"));
            next.set_sensitive(index + 1 < findings.len());
            let ctrl_next = self.clone();
            let list_next = findings.clone();
            next.connect_clicked(move |_| {
                let next_widget = ctrl_next.build_popover_content(list_next.clone(), index + 1);
                ctrl_next
                    .inner
                    .borrow()
                    .popover
                    .set_child(Some(&next_widget));
            });
            switcher.append(&prev);
            switcher.append(&count);
            switcher.append(&next);
            header.append(&switcher);
        }
        right.append(&header);

        let message = gtk::Label::new(None);
        message.set_markup(&format_message_markup(&finding.message));
        message.set_wrap(true);
        message.set_max_width_chars(34);
        message.set_xalign(0.0);
        message.add_css_class("writy-lint-message");
        right.append(&message);

        if finding.provider != "Harper · local" {
            let provider = gtk::Label::new(Some(&finding.provider));
            provider.set_xalign(0.0);
            provider.add_css_class("writy-meta");
            right.append(&provider);
        }

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let dismiss = gtk::Button::with_label("Dismiss");
        dismiss.add_css_class("writy-lint-dismiss");
        let ctrl = self.clone();
        let dismissed_finding = finding.clone();
        dismiss.connect_clicked(move |_| {
            ctrl.dismiss_finding(&dismissed_finding);
        });
        actions.append(&dismiss);

        if finding.can_add_to_dictionary() {
            let add_dict = gtk::Button::from_icon_name("list-add-symbolic");
            add_dict.add_css_class("writy-lint-dict-btn");
            add_dict.set_tooltip_text(Some(&format!(
                "Add \"{}\" to personal dictionary",
                finding.original
            )));
            add_dict.update_property(&[gtk::accessible::Property::Label("Add to dictionary")]);
            let ctrl = self.clone();
            let word = finding.original.clone();
            add_dict.connect_clicked(move |_| {
                ctrl.add_word_to_dictionary(&word);
            });
            actions.append(&add_dict);
        }
        right.append(&actions);

        if !finding.rule.is_empty() {
            right.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            let disable = gtk::Button::with_label(&format!("Disable rule {}", finding.rule));
            disable.add_css_class("writy-lint-disable");
            disable.set_halign(gtk::Align::Start);
            let ctrl = self.clone();
            let rule = finding.rule.clone();
            disable.connect_clicked(move |_| {
                ctrl.disable_rule(&rule);
            });
            right.append(&disable);
        }

        root.append(&right);
        root
    }

    fn apply_replacement(&self, start: usize, end: usize, original: &str, replacement: &str) {
        let (editor, popover) = {
            let inner = self.inner.borrow();
            (inner.editor.clone(), inner.popover.clone())
        };
        popover.popdown();
        let buffer = editor.buffer();
        let plain = plain_buffer_text(&buffer);
        if slice_chars(&plain, start, end).as_deref() != Some(original) {
            return;
        }
        let mut begin = buffer_iter_at_plain_offset(&buffer, start);
        let mut finish = buffer_iter_at_plain_offset(&buffer, end);
        buffer.begin_user_action();
        buffer.delete(&mut begin, &mut finish);
        buffer.insert(&mut begin, replacement);
        buffer.end_user_action();
        editor.grab_focus();
        self.check_now();
    }

    fn dismiss_finding(&self, finding: &EditorFinding) {
        let (editor, popover, updated) = {
            let mut inner = self.inner.borrow_mut();
            let key = finding.dismiss_key();
            if !inner.dismissed.contains(&key) {
                inner.dismissed.push(key);
            }
            let mut updated = false;
            if let Some(hash) = finding.ignore_hash {
                let mut cfg = Config::load().unwrap_or_default();
                cfg.harper = inner.harper.clone();
                if cfg.harper.add_ignored_hash(hash) {
                    let _ = cfg.save();
                    inner.harper = cfg.harper;
                    send(&inner.tx, Command::Reload);
                    updated = true;
                }
            }
            if let Some(suggestion) = &finding.result_suggestion {
                send(
                    &inner.tx,
                    Command::DismissSuggestion {
                        generation: finding.result_generation,
                        suggestion: suggestion.clone(),
                    },
                );
            }
            let plain = plain_buffer_text(&inner.editor.buffer());
            refresh_tags(&mut inner, &plain);
            (inner.editor.clone(), inner.popover.clone(), updated)
        };
        popover.popdown();
        if updated {
            self.check_now();
        }
        editor.grab_focus();
    }

    fn add_word_to_dictionary(&self, word: &str) {
        let (editor, popover, updated) = {
            let mut inner = self.inner.borrow_mut();
            let mut cfg = Config::load().unwrap_or_default();
            cfg.harper = inner.harper.clone();
            let updated = if cfg.harper.add_dictionary_word(word) {
                let _ = cfg.save();
                inner.harper = cfg.harper;
                send(&inner.tx, Command::Reload);
                true
            } else {
                false
            };
            (inner.editor.clone(), inner.popover.clone(), updated)
        };
        popover.popdown();
        if updated {
            self.check_now();
        }
        editor.grab_focus();
    }

    fn disable_rule(&self, rule: &str) {
        let (editor, popover, updated) = {
            let mut inner = self.inner.borrow_mut();
            let mut cfg = Config::load().unwrap_or_default();
            cfg.harper = inner.harper.clone();
            let updated = if cfg.harper.disable_rule(rule) {
                let _ = cfg.save();
                inner.harper = cfg.harper;
                send(&inner.tx, Command::Reload);
                true
            } else {
                false
            };
            (inner.editor.clone(), inner.popover.clone(), updated)
        };
        popover.popdown();
        if updated {
            self.check_now();
        }
        editor.grab_focus();
    }
}

fn spawn_worker() -> WorkerState {
    let slot = Arc::new((Mutex::new(WorkerSlot::default()), Condvar::new()));
    let worker_slot = slot.clone();
    let (res_tx, res_rx) = mpsc::sync_channel::<(u64, String, Vec<LintFinding>)>(8);
    std::thread::Builder::new()
        .name("writy-editor-lint".into())
        .spawn(move || {
            let mut checker = EnglishChecker::default();
            loop {
                let item = {
                    let Ok(mut guard) = worker_slot.0.lock() else {
                        break;
                    };
                    while guard.pending.is_none() && !guard.closed {
                        guard = match worker_slot.1.wait(guard) {
                            Ok(g) => g,
                            Err(_) => return,
                        };
                    }
                    if guard.closed {
                        break;
                    }
                    guard.pending.take()
                };
                let Some((seq, config, text)) = item else {
                    continue;
                };
                checker.configure(&config);
                let findings = checker.check_findings(&text);
                if res_tx.send((seq, text, findings)).is_err() {
                    break;
                }
            }
        })
        .expect("editor lint worker");
    WorkerState {
        slot,
        rx: res_rx,
        polling: false,
    }
}

fn ensure_tags(buffer: &gtk::TextBuffer) {
    let table = buffer.tag_table();
    for (name, fg, bg, underline) in [
        (
            TAG_SPELLING,
            gdk::RGBA::new(0.957, 0.247, 0.369, 1.0),
            gdk::RGBA::new(0.957, 0.247, 0.369, 0.14),
            gtk::pango::Underline::Single,
        ),
        (
            TAG_GRAMMAR,
            gdk::RGBA::new(0.659, 0.333, 0.969, 1.0),
            gdk::RGBA::new(0.659, 0.333, 0.969, 0.14),
            gtk::pango::Underline::Single,
        ),
        (
            TAG_IMPROVEMENT,
            gdk::RGBA::new(0.055, 0.647, 0.914, 1.0),
            gdk::RGBA::new(0.055, 0.647, 0.914, 0.13),
            gtk::pango::Underline::Single,
        ),
    ] {
        if table.lookup(name).is_none() {
            let tag = gtk::TextTag::new(Some(name));
            tag.set_underline(underline);
            tag.set_underline_rgba(Some(&fg));
            tag.set_background_rgba(Some(&bg));
            table.add(&tag);
        }
    }
}

fn clear_all_tags(buffer: &gtk::TextBuffer) {
    let start = buffer.start_iter();
    let end = buffer.end_iter();
    for name in [TAG_SPELLING, TAG_GRAMMAR, TAG_IMPROVEMENT] {
        buffer.remove_tag_by_name(name, &start, &end);
    }
}

fn convert_findings(text: &str, findings: Vec<LintFinding>) -> Vec<EditorFinding> {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    findings
        .into_iter()
        .filter(|f| f.start <= f.end && f.end <= len)
        .map(|f| {
            let (vis_start, vis_end) = visual_span(f.start, f.end, len);
            let original: String = chars[f.start..f.end].iter().collect();
            EditorFinding {
                start: f.start,
                end: f.end,
                vis_start,
                vis_end,
                original,
                kind: f.kind,
                category: f.category,
                rule: f.rule,
                message: f.message,
                replacements: f.replacements,
                provider: f.provider,
                result_suggestion: None,
                result_generation: 0,
                ignore_hash: Some(f.ignore_hash),
            }
        })
        .collect()
}

fn refresh_tags(inner: &mut Inner, plain: &str) {
    let buffer = inner.editor.buffer();
    ensure_tags(&buffer);
    clear_all_tags(&buffer);

    let mut combined = Vec::new();
    for f in &inner.local_findings {
        if !inner.dismissed.contains(&f.dismiss_key()) {
            combined.push(f.clone());
        }
    }
    for f in &inner.ai_findings {
        if inner.dismissed.contains(&f.dismiss_key()) {
            continue;
        }
        let dup = combined.iter().any(|existing| {
            existing.start == f.start
                && existing.end == f.end
                && f.replacements
                    .first()
                    .is_some_and(|r| existing.replacements.contains(r))
        });
        if !dup {
            combined.push(f.clone());
        }
    }

    combined.sort_by_key(|f| {
        let kind_rank = match f.kind {
            SuggestionKind::Spelling => 0,
            SuggestionKind::Grammar => 1,
            SuggestionKind::Rewrite => 2,
        };
        (f.vis_start, kind_rank, f.vis_end)
    });

    let plain_len = plain.chars().count();
    for f in &combined {
        if f.vis_start < f.vis_end && f.vis_end <= plain_len {
            let start = buffer_iter_at_plain_offset(&buffer, f.vis_start);
            let end = buffer_iter_at_plain_offset(&buffer, f.vis_end);
            buffer.apply_tag_by_name(f.tag_name(), &start, &end);
        }
    }
    inner.active_findings = combined;
}

fn visual_span(start: usize, end: usize, text_len: usize) -> (usize, usize) {
    if start < end {
        (start, end)
    } else if start > 0 {
        (start - 1, start)
    } else if end < text_len {
        (end, end + 1)
    } else {
        (start, end)
    }
}

fn plain_buffer_text(buffer: &gtk::TextBuffer) -> String {
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), false)
        .to_string()
}

fn slice_chars(text: &str, start: usize, end: usize) -> Option<String> {
    if start > end {
        return None;
    }
    let count = text.chars().count();
    if end > count {
        return None;
    }
    Some(text.chars().skip(start).take(end - start).collect())
}

fn buffer_iter_at_plain_offset(buffer: &gtk::TextBuffer, plain_offset: usize) -> gtk::TextIter {
    let plain_len = plain_buffer_text(buffer).chars().count();
    if buffer.char_count() as usize == plain_len {
        return buffer.iter_at_offset(plain_offset as i32);
    }
    let mut iter = buffer.start_iter();
    let mut count = 0;
    while count < plain_offset && !iter.is_end() {
        if iter.child_anchor().is_none() {
            count += 1;
        }
        iter.forward_char();
    }
    iter
}

fn plain_offset_at_iter(buffer: &gtk::TextBuffer, target: &gtk::TextIter) -> usize {
    let plain_len = plain_buffer_text(buffer).chars().count();
    if buffer.char_count() as usize == plain_len {
        return target.offset() as usize;
    }
    let mut iter = buffer.start_iter();
    let mut count = 0;
    let target_offset = target.offset();
    while iter.offset() < target_offset && !iter.is_end() {
        if iter.child_anchor().is_none() {
            count += 1;
        }
        iter.forward_char();
    }
    count
}

pub(super) fn format_message_markup(message: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    let mut current = String::new();
    for ch in message.chars() {
        if ch == '`' {
            let escaped = glib::markup_escape_text(&current);
            if in_code {
                out.push_str("<tt><b>");
                out.push_str(&escaped);
                out.push_str("</b></tt>");
            } else {
                out.push_str(&escaped);
            }
            current.clear();
            in_code = !in_code;
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        let escaped = glib::markup_escape_text(&current);
        if in_code {
            out.push('`');
        }
        out.push_str(&escaped);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_backticked_tokens_safely() {
        let markup = format_message_markup("Did you mean `Wether` & <other>?");
        assert_eq!(
            markup,
            "Did you mean <tt><b>Wether</b></tt> &amp; &lt;other&gt;?"
        );
    }

    #[test]
    fn visual_span_handles_insertions() {
        assert_eq!(visual_span(5, 5, 10), (4, 5));
        assert_eq!(visual_span(0, 0, 10), (0, 1));
        assert_eq!(visual_span(2, 6, 10), (2, 6));
    }
}

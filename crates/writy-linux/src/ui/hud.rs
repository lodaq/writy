//! One bounded reading surface: content determines section height, not the viewport.
use super::*;
use std::cell::Cell;

pub(super) struct HudSection {
    pub card: gtk::Box,
    pub body: gtk::Label,
    pub source: gtk::Label,
    pub copy_text: Rc<RefCell<String>>,
    pub copy: gtk::Button,
}

pub(super) struct HudStack {
    pub window: gtk::ApplicationWindow,
    pub scroll: gtk::ScrolledWindow,
    pub tabs: gtk::Notebook,
    pub sections: BTreeMap<&'static str, HudSection>,
    pub expand: gtk::ToggleButton,
    query: gtk::Label,
    status: gtk::Label,
    activity: gtk::Spinner,
    header: gtk::Box,
    content: gtk::Box,
    footer: gtk::Label,
    monitor_height: Rc<Cell<i32>>,
    query_text: RefCell<String>,
}

impl HudStack {
    pub fn new(app: &gtk::Application, tx: &Commands) -> Self {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("Writy — Writing assistant")
            .default_width(440)
            .build();
        window.add_css_class("writy-hud");
        if layer_shell_supported() {
            window.init_layer_shell();
            window.set_namespace(Some("writy"));
            window.set_layer(Layer::Overlay);
            window.set_keyboard_mode(KeyboardMode::None);
            window.set_anchor(Edge::Top, true);
            window.set_anchor(Edge::Right, true);
            window.set_exclusive_zone(0);
            window.set_margin(Edge::Right, 20);
        }
        let surface = gtk::Box::new(gtk::Orientation::Vertical, 0);
        surface.add_css_class("writy-hud-surface");
        let header = gtk::Box::new(gtk::Orientation::Vertical, 6);
        header.add_css_class("writy-hud-toolbar");
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        toolbar.add_css_class("writy-hud-header");
        let title = label("Writing assistant", "writy-title");
        title.set_hexpand(true);
        toolbar.append(&title);
        let expand = gtk::ToggleButton::new();
        expand.set_icon_name("view-fullscreen-symbolic");
        expand.set_focusable(false);
        expand.set_tooltip_text(Some("Expand reading area"));
        expand.update_property(&[gtk::accessible::Property::Label("Expand reading area")]);
        toolbar.append(&expand);
        for (icon, caption, command) in [
            (
                "window-new-symbolic",
                "Open in search · Alt+G",
                Command::Search,
            ),
            ("window-close-symbolic", "Dismiss panels", Command::Dismiss),
        ] {
            let button = gtk::Button::from_icon_name(icon);
            button.set_focusable(false);
            button.set_tooltip_text(Some(caption));
            button.update_property(&[gtk::accessible::Property::Label(caption)]);
            let tx = tx.clone();
            button.connect_clicked(move |_| send(&tx, command.clone()));
            toolbar.append(&button);
        }
        header.append(&toolbar);
        let query = label("", "writy-hud-query");
        query.set_selectable(true);
        query.set_focusable(false);
        query.set_wrap(false);
        query.set_ellipsize(gtk::pango::EllipsizeMode::End);
        header.append(&query);
        let status_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let activity = gtk::Spinner::new();
        status_row.append(&activity);
        let status = label("", "writy-meta");
        status_row.append(&status);
        header.append(&status_row);
        surface.append(&header);

        let sections_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let tabs = gtk::Notebook::new();
        tabs.add_css_class("writy-hud-tabs");
        tabs.set_scrollable(true);
        tabs.set_show_border(false);
        sections_box.append(&tabs);
        let mut sections = BTreeMap::new();
        for (category, title) in CATEGORIES {
            let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
            card.add_css_class("writy-hud-section");
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("writy-hud-header");
            let heading = label(title, "writy-title");
            heading.set_hexpand(true);
            row.append(&heading);
            let copy_text = Rc::new(RefCell::new(String::new()));
            let copy = components::hud_copy_button(copy_text.clone(), title);
            copy.set_focusable(false);
            row.append(&copy);
            card.append(&row);
            let body = label("", "writy-body");
            body.set_selectable(true);
            body.set_focusable(false);
            body.set_yalign(0.);
            body.set_valign(gtk::Align::Start);
            body.set_ellipsize(gtk::pango::EllipsizeMode::None);
            card.append(&body);
            let source = label("", "writy-meta");
            source.set_selectable(true);
            source.set_focusable(false);
            source.set_yalign(0.);
            card.append(&source);
            tabs.append_page(&card, Some(&gtk::Label::new(Some(title))));
            sections.insert(
                category,
                HudSection {
                    card,
                    body,
                    source,
                    copy_text,
                    copy,
                },
            );
        }
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .min_content_height(1)
            .max_content_height(480)
            .propagate_natural_height(true)
            .child(&sections_box)
            .build();
        let adjustment = scroll.vadjustment();
        tabs.connect_switch_page(move |_, _, _| adjustment.set_value(0.));
        scroll.set_overlay_scrolling(false);
        surface.append(&scroll);
        let footer = label("Alt+G opens search", "writy-meta");
        footer.add_css_class("writy-hud-footer");
        surface.append(&footer);
        let weak_footer = footer.downgrade();
        let adjustment = scroll.vadjustment();
        adjustment.connect_changed(move |adjustment| {
            if let Some(footer) = weak_footer.upgrade() {
                footer.set_text(if adjustment.upper() > adjustment.page_size() + 1. {
                    "Scroll for more · Alt+G opens search"
                } else {
                    "Alt+G opens search"
                });
            }
        });
        // Pointer selection must not become a new PRIMARY lookup or hide the
        // panel through compositor resets. Keep keyboard input in the editor.
        let motion = gtk::EventControllerMotion::new();
        let enter_tx = tx.clone();
        motion.connect_enter(move |_, _, _| {
            send(&enter_tx, Command::HudInteraction { active: true })
        });
        let leave_tx = tx.clone();
        motion.connect_leave(move |_| send(&leave_tx, Command::HudInteraction { active: false }));
        surface.add_controller(motion);
        let hidden_tx = tx.clone();
        window.connect_hide(move |_| send(&hidden_tx, Command::HudInteraction { active: false }));
        window.set_child(Some(&surface));
        let monitor_height = Rc::new(Cell::new(900));
        let height = monitor_height.clone();
        let scroller = scroll.clone();
        let layout_header = header.clone();
        let layout_footer = footer.clone();
        let layout_content = sections_box.clone();
        let weak = window.downgrade();
        expand.connect_toggled(move |button| {
            let budget = reading_height(height.get(), button.is_active());

            button.set_icon_name(if button.is_active() {
                "view-restore-symbolic"
            } else {
                "view-fullscreen-symbolic"
            });
            button.update_property(&[gtk::accessible::Property::Label(if button.is_active() {
                "Compact reading area"
            } else {
                "Expand reading area"
            })]);
            button.set_tooltip_text(Some(if button.is_active() {
                "Compact reading area"
            } else {
                "Expand reading area"
            }));
            if let Some(window) = weak.upgrade() {
                fit_reading_area(
                    &window,
                    &scroller,
                    &layout_header,
                    &layout_footer,
                    &layout_content,
                    window.default_width(),
                    budget,
                );
                // Expansion grows downward from the same edge, within the monitor.
                if layer_shell_supported() {
                    window.set_margin(
                        Edge::Top,
                        window
                            .margin(Edge::Top)
                            .clamp(8, (height.get() - budget - 8).max(8)),
                    );
                }
            }
        });
        if let Some(settings) = gtk::Settings::default() {
            if !settings.is_gtk_enable_animations() {
                window.add_css_class("writy-reduced-motion");
            }
            let weak = window.downgrade();
            settings.connect_gtk_enable_animations_notify(move |settings| {
                if let Some(window) = weak.upgrade() {
                    if settings.is_gtk_enable_animations() {
                        window.remove_css_class("writy-reduced-motion");
                    } else {
                        window.add_css_class("writy-reduced-motion");
                    }
                }
            });
        }
        Self {
            window,
            scroll,
            sections,
            tabs,
            expand,
            query,
            status,
            activity,
            header,
            content: sections_box,
            footer,
            monitor_height,
            query_text: RefCell::new(String::new()),
        }
    }

    pub fn update(
        &self,
        result: &Results,
        panels: &[Panel],
        config: &Config,
        monitor: Option<&gdk::Monitor>,
        bounds: gdk::Rectangle,
    ) {
        for (key, section) in &self.sections {
            let panel = panels
                .iter()
                .find(|panel| panel.category == *key)
                .filter(|_| config.huds.get(*key).copied().unwrap_or(true));
            section.card.set_visible(panel.is_some());
            if let Some(panel) = panel {
                // Background refreshes must not clear a user's text selection.
                if let Some(markup) = &panel.markup {
                    if section.body.label().as_str() != markup {
                        section.body.set_markup(markup);
                    }
                } else if section.body.text().as_str() != panel.body {
                    section.body.set_text(&panel.body);
                }
                if panel.copy_text.is_empty() {
                    section.card.add_css_class("writy-hud-empty");
                    section.body.add_css_class("writy-hud-pending");
                } else {
                    section.card.remove_css_class("writy-hud-empty");
                    section.body.remove_css_class("writy-hud-pending");
                }
                section.source.set_text(&panel.source);
                section.source.set_visible(!panel.source.is_empty());
                *section.copy_text.borrow_mut() = panel.copy_text.clone();
                section.copy.set_sensitive(!panel.copy_text.is_empty());
            }
        }
        if self.tabs.current_page().is_none() {
            self.tabs.set_current_page(Some(0));
        }
        let visible = self
            .sections
            .values()
            .any(|section| section.card.get_visible());
        if !visible {
            self.window.set_visible(false);
            return;
        }
        if *self.query_text.borrow() != result.snapshot.text {
            self.scroll.vadjustment().set_value(0.);
            *self.query_text.borrow_mut() = result.snapshot.text.clone();
        }
        self.query.set_text(&result.snapshot.text);
        self.query.set_tooltip_text(Some(&result.snapshot.text));
        self.status.set_text(if result.status.is_empty() {
            if result.writing {
                "While you write"
            } else {
                "Dictionary lookup"
            }
        } else {
            &result.status
        });
        self.activity.set_spinning(result.status == "Generating…");
        self.activity.set_visible(result.status == "Generating…");
        self.monitor_height.set(bounds.height());
        let width = (bounds.width() - 40).clamp(240, 440);
        let budget = reading_height(bounds.height(), self.expand.is_active());
        fit_reading_area(
            &self.window,
            &self.scroll,
            &self.header,
            &self.footer,
            &self.content,
            width,
            budget,
        );
        if layer_shell_supported() {
            if let Some(monitor) = monitor {
                self.window.set_monitor(Some(monitor));
            }
            if !result.writing
                && let Some((x, y, _, _)) = result.caret
            {
                let (left, top) =
                    pointer_placement((x, y), bounds, width, self.window.default_height());
                self.window.set_anchor(Edge::Right, false);
                self.window.set_anchor(Edge::Left, true);
                self.window.set_margin(Edge::Left, left);
                self.window.set_margin(Edge::Top, top);
            } else {
                self.window.set_anchor(Edge::Left, false);
                self.window.set_anchor(Edge::Right, true);
                let candidate = result
                    .caret
                    .map(|(_, y, _, h)| y - bounds.y() + h + 12)
                    .unwrap_or(16);
                let top = if candidate + budget <= bounds.height() {
                    candidate
                } else {
                    16
                };
                self.window.set_margin(
                    Edge::Top,
                    top.clamp(0, (bounds.height() - budget - 8).max(0)),
                );
            }
        }
        self.window.set_visible(true);
    }
}

fn fit_reading_area(
    window: &gtk::ApplicationWindow,
    scroll: &gtk::ScrolledWindow,
    header: &gtk::Box,
    footer: &gtk::Label,
    content: &gtk::Box,
    width: i32,
    budget: i32,
) {
    let chrome = header.measure(gtk::Orientation::Vertical, width).0
        + footer.measure(gtk::Orientation::Vertical, width).0
        + 4;
    let cap = (budget - chrome).max(48);
    let natural = content.measure(gtk::Orientation::Vertical, width - 16).1;
    let visible_content = natural.clamp(1, cap);
    scroll.set_min_content_height(1);
    scroll.set_max_content_height(cap);
    scroll.set_min_content_height(visible_content);
    window.set_default_size(width, chrome + visible_content);
}

fn pointer_placement(
    pointer: (i32, i32),
    bounds: gdk::Rectangle,
    width: i32,
    height: i32,
) -> (i32, i32) {
    let x = pointer.0.saturating_sub(bounds.x());
    let y = pointer.1.saturating_sub(bounds.y());
    let left = if x.saturating_add(width + 12) <= bounds.width() - 8 {
        x.saturating_add(12)
    } else {
        x.saturating_sub(width + 12)
    };
    let top = if y.saturating_add(height + 12) <= bounds.height() - 8 {
        y.saturating_add(12)
    } else {
        y.saturating_sub(height + 12)
    };
    (
        left.clamp(8, (bounds.width() - width - 8).max(8)),
        top.clamp(8, (bounds.height() - height - 8).max(8)),
    )
}

fn reading_height(monitor_height: i32, expanded: bool) -> i32 {
    let available = (monitor_height - 48).max(96);
    if expanded {
        available
    } else {
        (monitor_height * 72 / 100).clamp(240, 680).min(available)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_hud_stays_beside_pointer_and_inside_output() {
        let bounds = gdk::Rectangle::new(1920, -200, 1280, 800);
        assert_eq!(pointer_placement((2100, 0), bounds, 440, 400), (192, 212));
        assert_eq!(pointer_placement((3190, 590), bounds, 440, 400), (818, 378));
        let (x, y) = pointer_placement((-100, -1000), bounds, 440, 400);
        assert_eq!((x, y), (8, 8));
    }

    #[test]
    fn reading_area_stays_on_small_and_large_monitors() {
        for height in [200, 480, 700, 1080, 2160] {
            assert!(reading_height(height, false) <= height - 48);
            assert!(reading_height(height, true) <= height - 48);
            assert!(reading_height(height, true) >= reading_height(height, false));
        }
    }
}

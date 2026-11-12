//! A draggable reference window with persistent navigation and bounded pages.
use super::*;
use std::cell::Cell;

pub(super) struct HudSection {
    pub card: gtk::Box,
    pub scroll: gtk::ScrolledWindow,
    pub body: gtk::Label,
    pub source: gtk::Label,
    responses: gtk::Box,
    rendered: RefCell<Option<(Vec<SourceGroup>, String)>>,
    pub copy_text: Rc<RefCell<String>>,
    pub copy: gtk::Button,
}

struct Placement {
    bounds: Cell<gdk::Rectangle>,
    automatic: Cell<(i32, i32)>,
    manual: Cell<Option<(i32, i32)>>,
    pending_drag: Cell<Option<(i32, i32)>>,
}

pub(super) struct HudStack {
    pub window: gtk::ApplicationWindow,
    pub tabs: gtk::Notebook,
    pub sections: BTreeMap<&'static str, HudSection>,
    pub expand: gtk::ToggleButton,
    query: gtk::Label,
    status: gtk::Label,
    activity: gtk::Spinner,
    placement: Rc<Placement>,
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
            window.set_exclusive_zone(0);
        }
        let placement = Rc::new(Placement {
            bounds: Cell::new(gdk::Rectangle::new(0, 0, 900, 900)),
            automatic: Cell::new((440, 16)),
            manual: Cell::new(None),
            pending_drag: Cell::new(None),
        });
        let surface = gtk::Box::new(gtk::Orientation::Vertical, 0);
        surface.add_css_class("writy-hud-surface");
        let header = gtk::Box::new(gtk::Orientation::Vertical, 5);
        header.add_css_class("writy-hud-toolbar");
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        toolbar.add_css_class("writy-hud-header");
        let title = label("Writing assistant", "writy-title");
        title.set_wrap(false);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_hexpand(true);
        title.set_tooltip_text(Some("Drag the header to move this panel"));
        toolbar.append(&title);
        let expand = gtk::ToggleButton::new();
        expand.set_icon_name("view-fullscreen-symbolic");
        expand.set_focusable(false);
        expand.set_tooltip_text(Some("Fullscreen"));
        expand.update_property(&[gtk::accessible::Property::Label("Fullscreen")]);
        toolbar.append(&expand);
        for (icon, caption, command) in [
            (
                "window-new-symbolic",
                "Open in search · Alt+G",
                Command::Search { text: None },
            ),
            ("window-close-symbolic", "Dismiss panel", Command::Dismiss),
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
        query.set_wrap(false);
        query.set_ellipsize(gtk::pango::EllipsizeMode::End);
        header.append(&query);
        let status_row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let activity = gtk::Spinner::new();
        status_row.append(&activity);
        let status = label("", "writy-meta");
        status.set_wrap(false);
        status.set_ellipsize(gtk::pango::EllipsizeMode::End);
        status.set_hexpand(true);
        status_row.append(&status);
        header.append(&status_row);
        surface.append(&header);

        let tabs = gtk::Notebook::new();
        tabs.add_css_class("writy-hud-tabs");
        tabs.set_scrollable(true);
        tabs.set_show_border(false);
        tabs.set_vexpand(true);
        let mut sections = BTreeMap::new();
        for (category, title) in CATEGORIES {
            let card = gtk::Box::new(gtk::Orientation::Vertical, 7);
            card.add_css_class("writy-hud-section");
            card.set_valign(gtk::Align::Start);
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 5);
            row.add_css_class("writy-hud-header");
            let heading = label(title, "writy-title");
            heading.set_hexpand(true);
            row.append(&heading);
            let copy_text = Rc::new(RefCell::new(String::new()));
            let copy = components::hud_copy_button(copy_text.clone(), title);
            copy.set_focusable(false);
            row.append(&copy);
            card.append(&row);
            let source = label("", "writy-source-title");
            card.append(&source);
            let body = label("", "writy-body");
            body.set_selectable(true);
            body.set_focusable(false);
            body.set_yalign(0.);
            body.set_valign(gtk::Align::Start);
            card.append(&body);
            let responses = gtk::Box::new(gtk::Orientation::Vertical, 10);
            card.append(&responses);
            card.set_hexpand(true);
            let scroll = gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Never)
                .vscrollbar_policy(gtk::PolicyType::Automatic)
                .vexpand(true)
                .child(&card)
                .build();
            scroll.set_overlay_scrolling(false);
            let tab = components::tab_label(title);
            tabs.append_page(&scroll, Some(&tab));
            sections.insert(
                category,
                HudSection {
                    card,
                    scroll,
                    body,
                    source,
                    responses,
                    rendered: RefCell::new(None),
                    copy_text,
                    copy,
                },
            );
        }
        components::enable_wheel_tab_switching(&tabs);
        let cards: Vec<gtk::Box> = sections
            .values()
            .map(|section| section.card.clone())
            .collect();
        let last_width = Cell::new(0);
        let drag_placement = placement.clone();
        window.add_tick_callback(move |window, _| {
            apply_drag_position(window, &drag_placement);
            let width = window.width();
            if last_width.replace(width) != width {
                let margin = ((width - 960) / 2).max(0);
                for card in &cards {
                    card.set_margin_start(margin);
                    card.set_margin_end(margin);
                }
            }
            glib::ControlFlow::Continue
        });
        surface.append(&tabs);
        let footer = label(
            "Shift + scroll changes section · Alt+G opens search",
            "writy-meta",
        );
        footer.add_css_class("writy-hud-footer");
        surface.append(&footer);

        let wheel = gtk::EventControllerScroll::new(
            gtk::EventControllerScrollFlags::BOTH_AXES | gtk::EventControllerScrollFlags::DISCRETE,
        );
        wheel.set_propagation_phase(gtk::PropagationPhase::Capture);
        let notebook = tabs.clone();
        wheel.connect_scroll(move |controller, dx, dy| {
            if controller
                .current_event_state()
                .contains(gdk::ModifierType::SHIFT_MASK)
            {
                switch_section(&notebook, if dy != 0. { dy } else { dx });
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        surface.add_controller(wheel);
        let motion = gtk::EventControllerMotion::new();
        let enter_tx = tx.clone();
        motion.connect_enter(move |_, _, _| {
            send(&enter_tx, Command::HudInteraction { active: true })
        });
        let leave_tx = tx.clone();
        motion.connect_leave(move |_| send(&leave_tx, Command::HudInteraction { active: false }));
        surface.add_controller(motion);
        let hidden_tx = tx.clone();
        let hidden_placement = placement.clone();
        window.connect_hide(move |_| {
            hidden_placement.pending_drag.set(None);
            send(&hidden_tx, Command::HudInteraction { active: false });
        });
        window.set_child(Some(&surface));

        let weak = window.downgrade();
        let layout = placement.clone();
        expand.connect_toggled(move |button| {
            let full = button.is_active();
            let caption = if full { "Restore panel" } else { "Fullscreen" };
            button.set_icon_name(if full {
                "view-restore-symbolic"
            } else {
                "view-fullscreen-symbolic"
            });
            button.set_tooltip_text(Some(caption));
            button.update_property(&[gtk::accessible::Property::Label(caption)]);
            if let Some(window) = weak.upgrade() {
                apply_geometry(&window, &layout, full);
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
        install_drag(&header, &window, &expand, placement.clone());
        Self {
            window,
            tabs,
            sections,
            expand,
            query,
            status,
            activity,
            placement,
            query_text: RefCell::new(String::new()),
        }
    }

    #[cfg(test)]
    pub fn scroll(&self) -> gtk::ScrolledWindow {
        self.tabs
            .nth_page(self.tabs.current_page())
            .unwrap()
            .downcast()
            .unwrap()
    }

    pub fn update(
        &self,
        result: &Results,
        panels: &[Panel],
        config: &Config,
        monitor: Option<&gdk::Monitor>,
        bounds: gdk::Rectangle,
    ) {
        let first_query = self.query_text.borrow().is_empty();
        for (key, section) in &self.sections {
            let panel = panels
                .iter()
                .find(|p| p.category == *key)
                .filter(|_| config.huds.get(*key).copied().unwrap_or(true));
            section.scroll.set_visible(panel.is_some());
            section.card.set_visible(panel.is_some());
            if let Some(panel) = panel {
                let rendered = (panel.groups.clone(), panel.body.clone());
                if section.rendered.borrow().as_ref() != Some(&rendered) {
                    components::response_rows(
                        &section.source,
                        &section.body,
                        &section.responses,
                        panel,
                    );
                    *section.rendered.borrow_mut() = Some(rendered);
                }
                if panel.copy_text.is_empty() {
                    section.body.add_css_class("writy-hud-pending");
                } else {
                    section.body.remove_css_class("writy-hud-pending");
                }
                *section.copy_text.borrow_mut() = panel.copy_text.clone();
                section.copy.set_sensitive(!panel.copy_text.is_empty());
            }
        }
        if !self.sections.values().any(|s| s.card.get_visible()) {
            self.window.set_visible(false);
            return;
        }
        let new_selection = result.snapshot.source == "selection"
            && *self.query_text.borrow() != result.snapshot.text;
        if (first_query || new_selection) && self.sections["translation"].scroll.get_visible() {
            self.tabs.set_current_page(Some(0));
        }
        if self.tabs.current_page().is_none_or(|page| {
            self.tabs
                .nth_page(Some(page))
                .is_none_or(|w| !w.get_visible())
        }) {
            switch_section(&self.tabs, 1.);
        }
        if *self.query_text.borrow() != result.snapshot.text {
            for section in self.sections.values() {
                section.scroll.vadjustment().set_value(0.);
            }
            self.query.set_text(&result.snapshot.text);
            *self.query_text.borrow_mut() = result.snapshot.text.clone();
        }
        self.query.set_tooltip_text(Some(&result.snapshot.text));
        self.status.set_text(if !result.status.is_empty() {
            &result.status
        } else if result.writing {
            "While you write"
        } else {
            "Dictionary lookup"
        });
        self.activity.set_spinning(result.status == "Generating…");
        self.activity.set_visible(result.status == "Generating…");
        self.placement.bounds.set(bounds);
        let (width, height) = compact_size(bounds);
        let automatic = if !result.writing
            && let Some((x, y, _, _)) = result.caret
        {
            pointer_placement((x, y), bounds, width, height)
        } else {
            let top = result
                .caret
                .map(|(_, y, _, h)| y - bounds.y() + h + 12)
                .unwrap_or(16);
            (
                bounds.width() - width - 20,
                if top + height <= bounds.height() {
                    top
                } else {
                    16
                },
            )
        };
        self.placement.automatic.set(automatic);
        if layer_shell_supported()
            && let Some(monitor) = monitor
        {
            self.window.set_monitor(Some(monitor));
        }
        apply_geometry(&self.window, &self.placement, self.expand.is_active());
        self.window.set_visible(true);
    }
}

fn compact_size(bounds: gdk::Rectangle) -> (i32, i32) {
    (
        (bounds.width() - 40).clamp(240, 440),
        reading_height(bounds.height(), false),
    )
}

fn apply_geometry(window: &gtk::ApplicationWindow, placement: &Placement, full: bool) {
    let bounds = placement.bounds.get();
    let (width, height) = if full {
        (bounds.width(), bounds.height())
    } else {
        compact_size(bounds)
    };
    window.set_default_size(width, height);
    if full {
        window.add_css_class("writy-hud-fullscreen");
    } else {
        window.remove_css_class("writy-hud-fullscreen");
    }
    if layer_shell_supported() {
        for edge in [Edge::Top, Edge::Left] {
            window.set_anchor(edge, true);
        }
        for edge in [Edge::Bottom, Edge::Right] {
            window.set_anchor(edge, full);
            window.set_margin(edge, 0);
        }
        let (x, y) = if full {
            (0, 0)
        } else {
            clamp_position(
                placement.manual.get().unwrap_or(placement.automatic.get()),
                bounds,
                width,
                height,
            )
        };
        window.set_margin(Edge::Left, x);
        window.set_margin(Edge::Top, y);
    } else if full {
        window.fullscreen();
    } else {
        window.unfullscreen();
    }
}

fn clamp_position(
    (x, y): (i32, i32),
    bounds: gdk::Rectangle,
    width: i32,
    height: i32,
) -> (i32, i32) {
    (
        x.clamp(0, (bounds.width() - width).max(0)),
        y.clamp(0, (bounds.height() - height).max(0)),
    )
}

fn install_drag(
    header: &gtk::Box,
    window: &gtk::ApplicationWindow,
    expand: &gtk::ToggleButton,
    placement: Rc<Placement>,
) {
    header.set_cursor_from_name(Some("grab"));
    let drag = gtk::GestureDrag::new();
    drag.set_button(1);
    let weak = window.downgrade();
    let full = expand.clone();
    drag.connect_drag_begin(move |gesture, x, y| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let mut picked = gesture
            .widget()
            .and_then(|w| w.pick(x, y, gtk::PickFlags::DEFAULT));
        while let Some(widget) = picked {
            if widget.is::<gtk::Button>() {
                gesture.set_state(gtk::EventSequenceState::Denied);
                return;
            }
            picked = widget.parent();
        }
        if full.is_active() {
            gesture.set_state(gtk::EventSequenceState::Denied);
            return;
        }
        if !layer_shell_supported()
            && let Some(event) = gesture.current_event()
            && let Some(device) = event.device()
            && let Some(surface) = window.surface().and_downcast::<gdk::Toplevel>()
        {
            surface.begin_move(&device, 1, x, y, event.time());
        }
    });
    let weak = window.downgrade();
    let full = expand.clone();
    let movement = placement.clone();
    drag.connect_drag_update(move |_, dx, dy| {
        if full.is_active() || !layer_shell_supported() {
            return;
        }
        if let Some(window) = weak.upgrade() {
            // Offsets are local to the moving surface, so use its current position.
            movement.pending_drag.set(Some((
                window.margin(Edge::Left) + dx as i32,
                window.margin(Edge::Top) + dy as i32,
            )));
        }
    });
    let weak = window.downgrade();
    drag.connect_drag_end(move |_, _, _| {
        if let Some(window) = weak.upgrade() {
            apply_drag_position(&window, &placement);
        }
    });
    header.add_controller(drag);
}

fn apply_drag_position(window: &gtk::ApplicationWindow, placement: &Placement) {
    let Some(position) = placement.pending_drag.take() else {
        return;
    };
    let bounds = placement.bounds.get();
    let (width, height) = compact_size(bounds);
    let (x, y) = clamp_position(position, bounds, width, height);
    placement.manual.set(Some((x, y)));
    // Move once per frame without resetting size, anchors or fullscreen styling.
    if window.margin(Edge::Left) != x {
        window.set_margin(Edge::Left, x);
    }
    if window.margin(Edge::Top) != y {
        window.set_margin(Edge::Top, y);
    }
}

pub(super) fn switch_section(tabs: &gtk::Notebook, delta: f64) {
    components::switch_tab(tabs, delta);
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

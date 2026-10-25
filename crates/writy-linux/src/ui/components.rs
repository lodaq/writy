//! Shared native components for search, settings and floating panels.
use super::*;

pub(super) fn section(title: &str, description: &str) -> (gtk::Box, gtk::Box) {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 8);
    section.append(&label(title, "writy-title"));
    if !description.is_empty() {
        section.append(&label(description, "writy-meta"));
    }
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.add_css_class("writy-card");
    section.append(&content);
    (section, content)
}
pub(super) fn empty_state(title: &str, detail: &str) -> gtk::Box {
    let state = gtk::Box::new(gtk::Orientation::Vertical, 8);
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
pub(super) fn copy_button(text: Rc<RefCell<String>>, caption: &'static str) -> gtk::Button {
    let button = gtk::Button::with_label(caption);
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
    let card = gtk::Box::new(gtk::Orientation::Vertical, 12);
    card.add_css_class("writy-card");
    card.add_css_class("writy-result-section");
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let title = label(panel.title, "writy-title");
    title.set_hexpand(true);
    header.append(&title);
    let copy = copy_button(Rc::new(RefCell::new(panel.copy_text.clone())), "Copy");
    copy.set_sensitive(!panel.copy_text.is_empty());
    copy.set_tooltip_text(Some(&format!("Copy {}", panel.title.to_lowercase())));
    header.append(&copy);
    card.append(&header);
    let body = label(&panel.body, "writy-body");
    if let Some(markup) = &panel.markup {
        body.set_markup(markup);
    }
    body.set_selectable(true);
    card.append(&body);
    if !panel.source.is_empty() {
        card.append(&label(&panel.source, "writy-meta"));
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

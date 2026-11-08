//! Shared native components for search, settings and floating panels.
use super::*;

glib::wrapper! {
    pub struct ReadingColumn(ObjectSubclass<reading_column::ReadingColumn>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl ReadingColumn {
    pub fn new(child: &gtk::Widget, actions: &gtk::Box, header: &gtk::Box) -> Self {
        use gtk::subclass::prelude::ObjectSubclassIsExt;
        let column: Self = glib::Object::new();
        column.imp().child.set(child.clone()).unwrap();
        column.imp().actions.set(actions.clone()).unwrap();
        column.imp().header.set(header.clone()).unwrap();
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
    }
    fn inset(width: i32) -> i32 {
        (width / 18).clamp(20, 80)
    }
    impl ReadingColumn {
        fn arrange_actions(&self, width: i32) {
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
    let section = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let heading = label(title, "writy-title");
    heading.add_css_class("writy-section-title");
    section.append(&heading);
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
pub(super) fn tab_label(caption: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(caption));
    label.set_width_chars(caption.chars().count().min(12) as i32);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_tooltip_text(Some(caption));
    label
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
    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    card.add_css_class("writy-card");
    card.add_css_class("writy-result-section");
    let header = gtk::FlowBox::new();
    header.add_css_class("writy-result-header");
    header.set_selection_mode(gtk::SelectionMode::None);
    header.set_min_children_per_line(1);
    header.set_max_children_per_line(2);
    header.set_column_spacing(12);
    header.set_row_spacing(8);
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
        let source = gtk::Box::new(gtk::Orientation::Vertical, 4);
        source.add_css_class("writy-source-group");
        source.append(&label(&group.source, "writy-source-title"));
        for (index, row) in group.rows.iter().enumerate() {
            let entry = gtk::Box::new(gtk::Orientation::Horizontal, 14);
            entry.add_css_class("writy-result-entry");
            let body = label(row.search_text(), "writy-body");
            body.set_selectable(true);
            body.set_focusable(false);
            let spacing = gtk::pango::AttrList::new();
            spacing.insert(gtk::pango::AttrFloat::new_line_height(
                if body.direction() == gtk::TextDirection::Rtl {
                    0.76
                } else {
                    1.4
                },
            ));
            body.set_attributes(Some(&spacing));
            if let Some(markup) = row.search_markup() {
                body.set_markup(markup);
            }
            entry.set_direction(body.direction());
            if group.rows.len() > 1 {
                let number = label(&format!("{}.", index + 1), "writy-sense-number");
                number.set_xalign(1.);
                number.set_valign(gtk::Align::Start);
                entry.append(&number);
            }
            let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
            content.set_hexpand(true);
            if panel.category == "definition"
                && !row.headword.is_empty()
                && (index == 0
                    || row.headword != group.rows[index - 1].headword
                    || row.part_of_speech != group.rows[index - 1].part_of_speech)
            {
                let heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
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

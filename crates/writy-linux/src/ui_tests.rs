//! Opt-in runtime and visual checks; never starts the backend or captures real text.
use super::*;
use writy_core::types::{Attribution, Confidence, Language, LexicalEntry, Sense, TextSnapshot};

fn settle() {
    let context = glib::MainContext::default();
    let deadline = std::time::Instant::now() + Duration::from_millis(350);
    while std::time::Instant::now() < deadline {
        while context.pending() {
            context.iteration(false);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn resize(window: &impl IsA<gtk::Window>, width: i32, height: i32) {
    let window = window.as_ref();
    window.set_visible(false);
    window.set_default_size(width, height);
    window.present();
    settle();
    // Xvfb has no window manager to honor GTK resize requests after mapping.
    if std::env::var("WRITY_TEST_XVFB").as_deref() == Ok("1") {
        let status = std::process::Command::new("python3")
            .arg("-c")
            .arg(r#"
import ctypes as c, sys
x = c.CDLL('libX11.so.6')
x.XOpenDisplay.restype = c.c_void_p
d = x.XOpenDisplay(None)
assert d
x.XDefaultRootWindow.argtypes = [c.c_void_p]
x.XDefaultRootWindow.restype = c.c_ulong
x.XQueryTree.argtypes = [c.c_void_p, c.c_ulong, c.POINTER(c.c_ulong), c.POINTER(c.c_ulong), c.POINTER(c.POINTER(c.c_ulong)), c.POINTER(c.c_uint)]
x.XInternAtom.argtypes = [c.c_void_p, c.c_char_p, c.c_int]
x.XInternAtom.restype = c.c_ulong
x.XGetWindowProperty.argtypes = [c.c_void_p, c.c_ulong, c.c_ulong, c.c_long, c.c_long, c.c_int, c.c_ulong, c.POINTER(c.c_ulong), c.POINTER(c.c_int), c.POINTER(c.c_ulong), c.POINTER(c.c_ulong), c.POINTER(c.c_void_p)]
atom = x.XInternAtom(d, b'_NET_WM_NAME', 0)
x.XResizeWindow.argtypes = [c.c_void_p, c.c_ulong, c.c_uint, c.c_uint]
x.XSync.argtypes = [c.c_void_p, c.c_int]
x.XFree.argtypes = [c.c_void_p]
root, parent, children, count = c.c_ulong(), c.c_ulong(), c.POINTER(c.c_ulong)(), c.c_uint()
assert x.XQueryTree(d, x.XDefaultRootWindow(d), c.byref(root), c.byref(parent), c.byref(children), c.byref(count))
found = False
for window in children[:count.value]:
    kind, form, size, after, name = c.c_ulong(), c.c_int(), c.c_ulong(), c.c_ulong(), c.c_void_p()
    x.XGetWindowProperty(d, window, atom, 0, 4096, 0, 0, c.byref(kind), c.byref(form), c.byref(size), c.byref(after), c.byref(name))
    if name.value:
        if c.string_at(name, size.value).decode() == sys.argv[1]:
            x.XResizeWindow(d, window, int(sys.argv[2]), int(sys.argv[3]))
            found = True
        x.XFree(name)
x.XFree(children)
x.XSync(d, 0)
assert found, 'Synthetic test window not found'
"#)
            .args([window.title().unwrap().to_string(), width.to_string(), height.to_string()])
            .status().unwrap();
        assert!(status.success());
        settle();
    }
}

fn snapshot(window: &impl IsA<gtk::Window>, name: &str) {
    let window = window.as_ref();
    window.present();
    settle();
    let widget = window;
    let node = (0..8)
        .find_map(|_| {
            settle();
            let paintable = gtk::WidgetPaintable::new(Some(widget));
            let snapshot = gtk::Snapshot::new();
            paintable.snapshot(&snapshot, widget.width() as f64, widget.height() as f64);
            snapshot.to_node()
        })
        .unwrap_or_else(|| panic!("{name}: window has no rendered content"));
    let renderer = gtk::gsk::CairoRenderer::new();
    renderer.realize(window.surface().as_ref()).unwrap();
    let texture = renderer.render_texture(
        &node,
        Some(&gtk::graphene::Rect::new(
            0.,
            0.,
            widget.width() as f32,
            widget.height() as f32,
        )),
    );
    let directory = std::env::var("WRITY_UI_SNAPSHOTS").unwrap_or_else(|_| "/tmp/writy-ui".into());
    std::fs::create_dir_all(&directory).unwrap();
    texture
        .save_to_png(format!("{directory}/{name}.png"))
        .unwrap();
    renderer.unrealize();
}

fn collect_switches(widget: &gtk::Widget, output: &mut Vec<gtk::Switch>) {
    if let Some(toggle) = widget.downcast_ref::<gtk::Switch>() {
        output.push(toggle.clone());
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        collect_switches(&widget, output);
        child = widget.next_sibling();
    }
}

fn scroll_tab(tabs: &gtk::Notebook, delta: f64) {
    let controllers = tabs.first_child().unwrap().observe_controllers();
    let wheel = (0..controllers.n_items())
        .find_map(|i| {
            controllers
                .item(i)
                .and_downcast::<gtk::EventControllerScroll>()
        })
        .expect("wheel events must reach the tab header controller");
    wheel.emit_by_name::<bool>("scroll", &[&0f64, &delta]);
}

#[test]
#[ignore = "requires xvfb-run, Python 3 and libXtst"]
fn native_tab_wheel_switching() {
    gtk::init().unwrap();
    assert_eq!(
        gdk::Display::default().unwrap().type_().name(),
        "GdkX11Display",
        "Run with GDK_BACKEND=x11 xvfb-run -a"
    );
    let tabs = gtk::Notebook::new();
    for caption in ["First", "Second", "Third"] {
        tabs.append_page(
            &gtk::Label::new(Some("Content")),
            Some(&components::tab_label(caption)),
        );
    }
    components::enable_wheel_tab_switching(&tabs);
    let window = gtk::Window::builder()
        .title("Writy wheel test")
        .default_width(500)
        .default_height(300)
        .decorated(false)
        .child(&tabs)
        .build();
    window.present();
    settle();
    let scroll = |x: i32, y: i32, button: u32| {
        let status = std::process::Command::new("python3")
            .arg("-c")
            .arg(r#"
import ctypes, sys
x11 = ctypes.CDLL('libX11.so.6')
xtest = ctypes.CDLL('libXtst.so.6')
x11.XOpenDisplay.restype = ctypes.c_void_p
display = x11.XOpenDisplay(None)
assert display, 'X11 test display is unavailable'
xtest.XTestFakeMotionEvent.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_ulong]
xtest.XTestFakeButtonEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
x11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
xtest.XTestFakeMotionEvent(display, -1, int(sys.argv[1]), int(sys.argv[2]), 0)
for pressed in [1, 0]:
    xtest.XTestFakeButtonEvent(display, int(sys.argv[3]), pressed, 0)
x11.XSync(display, 0)
x11.XCloseDisplay(display)
"#)
            .args([x.to_string(), y.to_string(), button.to_string()])
            .status()
            .unwrap();
        assert!(status.success());
        settle();
    };
    let label = tabs.tab_label(&tabs.nth_page(Some(0)).unwrap()).unwrap();
    let tab = label.parent().unwrap();
    let bounds = tab.compute_bounds(&window).unwrap();
    let x = bounds.x() as i32 + 1;
    let y = (bounds.y() + bounds.height() / 2.) as i32;
    assert_eq!(
        window.pick(x as f64, y as f64, gtk::PickFlags::DEFAULT),
        Some(tab)
    );
    scroll(x, y, 5);
    assert_eq!(tabs.current_page(), Some(1));
    scroll(x, y, 4);
    assert_eq!(tabs.current_page(), Some(0));
    scroll(x, y, 4);
    assert_eq!(tabs.current_page(), Some(2));
    tabs.nth_page(Some(1)).unwrap().set_visible(false);
    scroll(x, y, 4);
    assert_eq!(tabs.current_page(), Some(0));
    scroll(250, 150, 5);
    assert_eq!(
        tabs.current_page(),
        Some(0),
        "Content scrolling must not switch tabs"
    );
    window.close();
}

fn contrast(widget: &gtk::Widget, background: gdk::RGBA) -> f64 {
    let foreground = widget.color();
    let channel = |front: f32, back: f32| {
        let value = f64::from(front * foreground.alpha() + back * (1. - foreground.alpha()));
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = 0.2126 * channel(foreground.red(), background.red())
        + 0.7152 * channel(foreground.green(), background.green())
        + 0.0722 * channel(foreground.blue(), background.blue());
    let base = |value: f32| {
        let value = f64::from(value);
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    let ground = 0.2126 * base(background.red())
        + 0.7152 * base(background.green())
        + 0.0722 * base(background.blue());
    (luminance.max(ground) + 0.05) / (luminance.min(ground) + 0.05)
}

#[test]
#[ignore = "requires a graphical session; renders synthetic data only"]
fn native_component_gallery() {
    gtk::init().unwrap();
    let tab = components::tab_label("Common word combinations");
    assert_eq!(tab.text(), "Common word combinations");
    assert_eq!(tab.ellipsize(), gtk::pango::EllipsizeMode::None);
    let mut ui_contrast = BTreeMap::new();
    arabic_result_reading_layout();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let captured = errors.clone();
    let provider = gtk::CssProvider::new();
    provider
        .connect_parsing_error(move |_, _, error| captured.borrow_mut().push(error.to_string()));
    provider.load_from_string(CSS);
    assert!(
        errors.borrow().is_empty(),
        "invalid GTK CSS: {:?}",
        errors.borrow()
    );
    let app = gtk::Application::builder()
        .application_id("io.github.lodaq.writy.visual-test")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(128);
    let view = View::new(&app, tx, Config::default());
    let search = view.borrow().search.clone();
    search.set_resizable(true);
    search.present();
    snapshot(&search, "search-empty");
    assert!(!view.borrow().copy.is_sensitive());
    let display = gtk::prelude::WidgetExt::display(&search);
    let primary = display.primary_clipboard();
    let copied = display.clipboard();
    copied.set_text("Copied sentence.\nSecond line.");
    primary.set_text("Selected sentence.\nSecond line.");
    view.borrow().open_search(None);
    settle();
    let buffer = view.borrow().editor.buffer();
    assert_eq!(
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), false),
        "Selected sentence.\nSecond line."
    );
    let mut queries = Vec::new();
    while let Ok((command, _)) = rx.try_recv() {
        if let Command::Query { text, translate } = command {
            queries.push((text, translate));
        }
    }
    assert_eq!(
        queries,
        vec![("Selected sentence.\nSecond line.".into(), true)],
        "Alt+G must translate the prefilled selection once, without a later lookup replacing it"
    );
    primary.set_content(None::<&gdk::ContentProvider>).unwrap();
    view.borrow().open_search(None);
    settle();
    assert_eq!(
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), false),
        "Copied sentence.\nSecond line."
    );
    snapshot(&search, "search-clipboard-input");
    copied.set_content(None::<&gdk::ContentProvider>).unwrap();
    view.borrow().editor.buffer().set_text("decision");
    let mut result = Results {
        writing: false,
        output_bounds: None,
        generation: 1,
        snapshot: TextSnapshot {
            source: "synthetic visual fixture".into(),
            focus_epoch: 1,
            revision: 1,
            language: Language::English,
            text: "decision".into(),
            confidence: Confidence::Projected,
        },
        entries: vec![LexicalEntry {
            headword: "decision".into(),
            senses: vec![Sense {
                definitions: vec!["A choice made after considering alternatives.".into()],
                translations: vec!["قرار".into()],
                examples: vec!["She made a decision.".into()],
                collocations: vec!["make a decision".into()],
                synonyms: vec!["choice".into()],
                antonyms: vec!["indecision".into()],
                word_family: vec!["decide · decisive".into()],
                ..Default::default()
            }],
            source: Attribution {
                name: "Sample dictionary".into(),
                license: "CC0".into(),
                url: String::new(),
            },
            ..Default::default()
        }],
        suggestions: vec![],
        translation: None,
        status: "Offline dictionary".into(),
        caret: None,
        output: None,
    };
    let mut second = result.entries[0].clone();
    second.source.name = "Second dictionary".into();
    second.senses[0].definitions = vec!["A conclusion reached after careful thought.".into()];
    result.entries.push(second);
    for panel in panels(&result)
        .into_iter()
        .filter(|panel| panel.category != "suggestions")
    {
        assert_eq!(
            panel.groups.len(),
            2,
            "{} must retain both sources",
            panel.title
        );
        assert_eq!(panel.groups[0].source, "Sample dictionary");
        assert_eq!(panel.groups[1].source, "Second dictionary");
    }
    view.borrow_mut().show(result.clone());
    assert!(view.borrow().copy.is_sensitive());
    view.borrow().copy.emit_clicked();
    assert_eq!(view.borrow().copy.label().as_deref(), Some("Copied"));
    assert!(
        view.borrow().results.is::<gtk::Notebook>(),
        "result categories must be tabs"
    );
    snapshot(&search, "search-results");
    let tabs = view.borrow().results.clone();
    assert_eq!(
        tabs.current_page(),
        Some(0),
        "Translation is the default search tab"
    );
    assert!(tabs.n_pages() >= 6);
    let page = tabs
        .nth_page(Some(0))
        .unwrap()
        .downcast::<gtk::ScrolledWindow>()
        .unwrap();
    let viewport = page.child().unwrap().downcast::<gtk::Viewport>().unwrap();
    let card = viewport.child().unwrap().downcast::<gtk::Box>().unwrap();
    fn find_label(widget: &gtk::Widget, class: &str) -> Option<gtk::Label> {
        if let Some(label) = widget.downcast_ref::<gtk::Label>()
            && label.has_css_class(class)
        {
            return Some(label.clone());
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            if let Some(label) = find_label(&widget, class) {
                return Some(label);
            }
            child = widget.next_sibling();
        }
        None
    }
    let body = find_label(card.upcast_ref(), "writy-body").unwrap();
    assert!(body.wraps());
    assert_eq!(body.ellipsize(), gtk::pango::EllipsizeMode::None);
    let header = card
        .first_child()
        .unwrap()
        .downcast::<gtk::FlowBox>()
        .unwrap();
    let copy = header
        .child_at_index(1)
        .unwrap()
        .child()
        .unwrap()
        .downcast::<gtk::Button>()
        .unwrap();
    copy.emit_clicked();
    assert_eq!(copy.label().as_deref(), Some("Copied"));
    let definition = (0..tabs.n_pages())
        .find(|index| {
            tabs.tab_label_text(&tabs.nth_page(Some(*index)).unwrap())
                .as_deref()
                == Some("Definitions")
        })
        .unwrap();
    tabs.set_current_page(Some(definition));
    scroll_tab(&tabs, 1.);
    assert_eq!(
        tabs.current_page(),
        Some((definition + 1) % tabs.n_pages()),
        "The mouse wheel over a tab must select the next result"
    );
    tabs.set_current_page(Some(definition));
    assert!(view.borrow().search_copy_text.borrow().contains("choice"));
    view.borrow_mut().show(result.clone());
    assert_eq!(tabs.current_page(), Some(definition));
    snapshot(&search, "search-definition-tab");
    tabs.set_current_page(Some(tabs.n_pages() - 1));
    assert!(
        !view.borrow().copy.is_sensitive(),
        "Attribution is not a writing suggestion"
    );
    snapshot(&search, "search-sources-tab");
    for index in 0..tabs.n_pages() {
        let tab = tabs
            .tab_label(&tabs.nth_page(Some(index)).unwrap())
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap();
        assert_eq!(tab.ellipsize(), gtk::pango::EllipsizeMode::None);
        if tab.is_mapped() {
            assert!(
                tab.height() >= tab.layout().pixel_size().1,
                "Category captions must fit within the tab bar"
            );
        }
    }
    tabs.set_current_page(Some(definition));
    {
        let mut arabic = result.clone();
        arabic.snapshot.text = "لعب".into();
        arabic.snapshot.language = Language::Arabic;
        let first = LexicalEntry {
            headword: "لَعِبَ".into(),
            definition_markup: Some(
                "<b>فعل</b> [لعب]\nثلاثي\nلازم\nيتصرف في: الماضي، المضارع، الأمر، المؤكد".into(),
            ),
            language: Language::Arabic,
            source: Attribution {
                name: "Arramooz Alwaseet".into(),
                license: "GPL-2.0".into(),
                url: "https://sourceforge.net/projects/arramooz/files/".into(),
            },
            senses: vec![Sense {
                part_of_speech: "فعل".into(),
                definitions: vec![
                    "فعل [لعب]\nثلاثي\nلازم\nيتصرف في: الماضي، المضارع، الأمر، المؤكد".into(),
                ],
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut second = first.clone();
        second.headword = "لَعِبَ بالألفاظ".into();
        second.senses[0].part_of_speech.clear();
        second.senses[0].definitions = vec!["استخدم الكلمات بطرق مختلفة.".into()];
        arabic.entries = vec![first.clone(), first, second];
        arabic.status.clear();
        view.borrow().editor.buffer().set_text("لعب");
        view.borrow_mut().show(arabic.clone());
        let definition = (0..tabs.n_pages())
            .find(|index| {
                tabs.tab_label_text(&tabs.nth_page(Some(*index)).unwrap())
                    .as_deref()
                    == Some("Definitions")
            })
            .unwrap();
        tabs.set_current_page(Some(definition));
        resize(&search, 1402, 1122);
        settle();
        // Match the target's capture size after GTK's native frame insets.
        resize(
            &search,
            1402 + 1402 - search.width(),
            1122 + 1122 - search.height(),
        );
        let input = view.borrow().editor.parent().unwrap().parent().unwrap();
        let split = input.parent().unwrap().downcast::<gtk::Box>().unwrap();
        assert_eq!(split.orientation(), gtk::Orientation::Horizontal);
        assert!((input.width() - view.borrow().content.width()).abs() <= 1);
        snapshot(&search, "search-arabic-wide");
        let page = tabs.nth_page(Some(definition)).unwrap();
        let body = find_label(&page, "writy-body").unwrap();
        assert!(
            body.width() > 400 && body.width() < search.width() / 2,
            "Wide search results must occupy half the window: body={}, window={}",
            body.width(),
            search.width()
        );
        assert_eq!(body.direction(), gtk::TextDirection::Rtl);
        assert!(body.text().contains("ثلاثي · لازم"));
        assert!(!body.uses_markup());
        assert!(
            view.borrow()
                .search_copy_text
                .borrow()
                .contains("ثلاثي\nلازم")
        );
        // GTK 4.12 has no replacement for resolving named theme colors.
        #[allow(deprecated)]
        let background = body.style_context().lookup_color("writy_surface").unwrap();
        let source = find_label(&page, "writy-source-title").unwrap();
        let body_contrast = contrast(body.upcast_ref(), background);
        let source_contrast = contrast(source.upcast_ref(), background);
        assert!(
            body_contrast >= 4.5 && source_contrast >= 4.5,
            "Body={body_contrast}, attribution={source_contrast}"
        );
        let directory =
            std::env::var("WRITY_UI_SNAPSHOTS").unwrap_or_else(|_| "/tmp/writy-ui".into());
        std::fs::write(
            format!("{directory}/contrast.json"),
            serde_json::to_string_pretty(
                &serde_json::json!({"body":body_contrast,"source":source_contrast}),
            )
            .unwrap(),
        )
        .unwrap();
        let (offset, _) = body.layout_offsets();
        assert!(
            body.layout().index_to_pos(0).x() / gtk::pango::SCALE + offset > body.width() * 3 / 4
        );
        resize(&search, 520, 760);
        assert_eq!(split.orientation(), gtk::Orientation::Vertical);
        assert!((input.height() - view.borrow().content.height()).abs() <= 1);
        snapshot(&search, "search-arabic-narrow");
        assert!(body.width() <= search.width());
        let scroll = tabs
            .nth_page(Some(definition))
            .unwrap()
            .downcast::<gtk::ScrolledWindow>()
            .unwrap();
        let adjustment = scroll.vadjustment();
        adjustment.set_value(adjustment.upper() - adjustment.page_size());
        snapshot(&search, "search-arabic-narrow-bottom");
        assert!(
            adjustment.value() > 0. || adjustment.upper() <= adjustment.page_size() + 1.,
            "All Arabic senses must fit or remain reachable by scrolling"
        );
        adjustment.set_value(0.);
        let settings = gtk::Settings::default().unwrap();
        let font = settings.gtk_font_name();
        settings.set_gtk_font_name(Some("Sans 20"));
        snapshot(&search, "search-arabic-large-text");
        let outer = search
            .child()
            .unwrap()
            .downcast::<gtk::ScrolledWindow>()
            .unwrap();
        let position = outer.vadjustment();
        position.set_value(position.upper() - position.page_size());
        snapshot(&search, "search-arabic-large-text-results");
        assert!(
            position.value() > 0. || position.upper() <= position.page_size() + 1.,
            "Large text must fit or scroll instead of clipping results or the footer"
        );
        adjustment.set_value(adjustment.upper() - adjustment.page_size());
        snapshot(&search, "search-arabic-large-text-bottom");
        assert!(adjustment.value() > 0. || adjustment.upper() <= adjustment.page_size() + 1.);
        settings.set_gtk_font_name(font.as_deref());
        resize(&search, 680, 760);
        view.borrow().editor.buffer().set_text("decision");
        view.borrow_mut().show(result.clone());
    }
    view.borrow_mut().clear();
    assert!(!view.borrow().copy.is_sensitive());
    let state = view.borrow().empty.first_child().unwrap();
    let heading = state
        .first_child()
        .unwrap()
        .next_sibling()
        .unwrap()
        .downcast::<gtk::Label>()
        .unwrap();
    assert_eq!(heading.text(), "Ready to look up");
    view.borrow_mut().show(result.clone());
    // Settings is a single reusable window, with Save outside the scroll area.
    view.borrow().settings();
    view.borrow().settings();
    let settings: Vec<_> = app
        .windows()
        .into_iter()
        .filter(|w| w.has_css_class("writy-settings"))
        .collect();
    assert_eq!(settings.len(), 1);
    let titlebar = settings[0]
        .titlebar()
        .unwrap()
        .downcast::<gtk::HeaderBar>()
        .unwrap();
    let title = titlebar
        .title_widget()
        .unwrap()
        .downcast::<gtk::Label>()
        .unwrap();
    assert!(!title.wraps(), "Settings title must remain on one line");
    snapshot(&settings[0], "settings");
    let description = find_label(settings[0].upcast_ref(), "writy-meta").unwrap();
    #[allow(deprecated)]
    let settings_background = description
        .style_context()
        .lookup_color("writy_surface")
        .unwrap();
    ui_contrast.insert(
        "settings_description",
        contrast(description.upcast_ref(), settings_background),
    );
    let shell = settings[0].child().unwrap().downcast::<gtk::Box>().unwrap();
    let preferences = shell
        .first_child()
        .unwrap()
        .downcast::<gtk::Notebook>()
        .unwrap();
    assert_eq!(preferences.n_pages(), 6);
    scroll_tab(&preferences, 1.);
    assert_eq!(
        preferences.current_page(),
        Some(1),
        "The mouse wheel over a settings tab must select the next page"
    );
    preferences.set_current_page(Some(0));
    let mut general = Vec::new();
    collect_switches(&preferences.nth_page(Some(0)).unwrap(), &mut general);
    assert_eq!(general.len(), 3);
    let mut privacy = Vec::new();
    collect_switches(&preferences.nth_page(Some(1)).unwrap(), &mut privacy);
    let offline = privacy[0].is_active();
    privacy[0].set_active(!offline);
    preferences.set_current_page(Some(2));
    snapshot(&settings[0], "settings-models");
    preferences.set_current_page(Some(3));
    snapshot(&settings[0], "settings-translation");
    preferences.set_current_page(Some(4));
    snapshot(&settings[0], "settings-panels");
    preferences.set_current_page(Some(5));
    snapshot(&settings[0], "settings-dictionaries");
    preferences.set_current_page(Some(0));
    assert_eq!(
        privacy[0].is_active(),
        !offline,
        "Tab changes must preserve edits"
    );
    settings[0].close();
    assert!(!settings[0].is_visible());
    view.borrow().settings();
    assert!(settings[0].is_visible());
    assert_eq!(
        privacy[0].is_active(),
        !offline,
        "Dismissal must preserve drafts"
    );
    let footer = shell.last_child().unwrap();
    let save = footer
        .last_child()
        .unwrap()
        .downcast::<gtk::Button>()
        .unwrap();
    assert!(save.grab_focus(), "Save must remain keyboard accessible");
    save.emit_clicked();
    assert_eq!(Config::load().unwrap().offline, !offline);
    snapshot(&settings[0], "settings-saved");
    privacy[0].set_active(offline);
    save.emit_clicked();
    footer
        .first_child()
        .unwrap()
        .downcast::<gtk::Label>()
        .unwrap()
        .set_text("");
    settings[0].set_resizable(true);
    resize(&settings[0], 400, 640);
    snapshot(&settings[0], "settings-narrow");
    assert!(settings[0].width() <= 400);
    let desktop = gtk::Settings::default().unwrap();
    let font = desktop.gtk_font_name();
    desktop.set_gtk_font_name(Some("Sans 20"));
    snapshot(&settings[0], "settings-large-text");
    assert!(
        settings[0].width() <= 400,
        "Settings must reflow at larger text sizes"
    );
    for (index, name) in [
        (2, "models"),
        (3, "translation"),
        (4, "panels"),
        (5, "dictionaries"),
    ] {
        preferences.set_current_page(Some(index));
        let page = preferences
            .nth_page(Some(index))
            .unwrap()
            .downcast::<gtk::ScrolledWindow>()
            .unwrap();
        snapshot(&settings[0], &format!("settings-{name}-large-text"));
        assert!(
            settings[0].width() <= 400,
            "{name} must fit with large text"
        );
        let position = page.vadjustment();
        position.set_value(position.upper() - position.page_size());
        snapshot(&settings[0], &format!("settings-{name}-large-text-bottom"));
        assert!(
            position.value() > 0.,
            "All {name} controls must remain reachable"
        );
    }
    desktop.set_gtk_font_name(font.as_deref());
    settings[0].set_visible(false);
    // Exercise every dictionary state without reading or modifying user data.
    {
        use writy_core::dictionary::DictionarySource;
        let list = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let message = label("", "writy-status");
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        let sources = [true, false]
            .into_iter()
            .map(|enabled| DictionarySource {
                id: format!("fixture-{enabled}"),
                attribution: Attribution {
                    name: if enabled {
                        "GNU GCIDE"
                    } else {
                        "Princeton WordNet"
                    }
                    .into(),
                    url: if enabled {
                        "https://gcide.gnu.org.ua/download"
                    } else {
                        "https://wordnet.princeton.edu/"
                    }
                    .into(),
                    license: "CC0".into(),
                },
                entries: 1,
                enabled,
            })
            .collect::<Vec<_>>();
        dictionaries::render(&list, &tx, &message, &sources);
        fn labels(widget: &gtk::Widget, output: &mut String) {
            if let Some(label) = widget.downcast_ref::<gtk::Label>() {
                output.push_str(&label.text());
                output.push('\n');
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                labels(&widget, output);
                child = widget.next_sibling();
            }
        }
        let mut text = String::new();
        labels(list.upcast_ref(), &mut text);
        assert_eq!(dictionaries::CATALOGUE.len(), 8);

        assert_eq!(text.lines().filter(|text| *text == "Remove").count(), 2);
        fn action(widget: &gtk::Widget, name: &str, title: &str) -> Option<gtk::Button> {
            if widget.has_css_class("writy-dictionary-row")
                && widget
                    .first_child()
                    .and_then(|child| child.downcast::<gtk::Label>().ok())
                    .is_some_and(|label| label.text() == name)
            {
                let actions = widget.last_child()?.downcast::<gtk::FlowBox>().ok()?;
                for index in 0..3 {
                    if let Some(button) = actions
                        .child_at_index(index)
                        .and_then(|child| child.child())
                        .and_downcast::<gtk::Button>()
                        && button.label().as_deref() == Some(title)
                    {
                        return Some(button);
                    }
                }
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                if let Some(button) = action(&widget, name, title) {
                    return Some(button);
                }
                child = widget.next_sibling();
            }
            None
        }
        assert!(action(list.upcast_ref(), "GNU GCIDE", "Download").is_none());
        assert!(
            action(list.upcast_ref(), "GNU GCIDE", "Remove")
                .unwrap()
                .is_sensitive()
        );
        assert!(
            action(list.upcast_ref(), "Arramooz Alwaseet", "Download")
                .unwrap()
                .is_sensitive()
        );
        assert!(action(list.upcast_ref(), "Arramooz Alwaseet", "Remove").is_none());
        assert_eq!(
            text.lines().filter(|text| *text == "Download").count(),
            dictionaries::CATALOGUE.len() - 2,
            "Uninstalled catalogue dictionaries need a one-click installer"
        );
        assert!(
            !text.contains("Import…"),
            "Catalogue setup must have only one action"
        );
        assert!(text.contains("Installed · Enabled · 1 entries"));
        assert!(text.contains("Installed · Disabled · 1 entries"));
        let preview = gtk::ApplicationWindow::builder()
            .application(&app)
            .title("Dictionary catalogue")
            .default_width(620)
            .default_height(640)
            .build();
        preview.add_css_class("writy-settings");
        preview.set_child(Some(&gtk::ScrolledWindow::builder().child(&list).build()));
        snapshot(&preview, "dictionary-catalogue");
        let download = action(list.upcast_ref(), "Arramooz Alwaseet", "Download").unwrap();
        #[allow(deprecated)]
        let button_background = download
            .style_context()
            .lookup_color("writy_button_surface")
            .unwrap();
        ui_contrast.insert(
            "dictionary_download",
            contrast(download.child().unwrap().upcast_ref(), button_background),
        );
        for (name, caption) in [
            ("dictionary_remove", "Remove"),
            ("dictionary_link", "Source & licence"),
        ] {
            let button = action(list.upcast_ref(), "GNU GCIDE", caption).unwrap();
            ui_contrast.insert(
                name,
                contrast(
                    button.child().unwrap().upcast_ref(),
                    if caption == "Remove" {
                        button_background
                    } else {
                        settings_background
                    },
                ),
            );
        }
        let scroll = preview
            .child()
            .unwrap()
            .downcast::<gtk::ScrolledWindow>()
            .unwrap();
        let position = scroll.vadjustment();
        position.set_value(position.upper() - position.page_size());
        snapshot(&preview, "dictionary-catalogue-installed");
        assert!(
            !text.contains("writy dict"),
            "Dictionary setup must not require terminal commands"
        );
        let directory = crate::ipc::runtime_dir()
            .unwrap()
            .join("gui-import-fixture");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("fixture.ifo");
        let mut index = "كتاب".as_bytes().to_vec();
        index.push(0);
        index.extend_from_slice(&0_u32.to_be_bytes());
        index.extend_from_slice(&6_u32.to_be_bytes());
        std::fs::write(directory.join("fixture.idx"), &index).unwrap();
        std::fs::write(directory.join("fixture.dict"), b"A book").unwrap();
        std::fs::write(&path, format!("StarDict's dict ifo file\nversion=2.4.2\nbookname=Fixture\nwordcount=1\nidxfilesize={}\nsametypesequence=m\n", index.len())).unwrap();
        crate::ipc::open_dictionary().unwrap();
        dictionaries::render(&list, &tx, &message, &[]);
        text.clear();
        labels(list.upcast_ref(), &mut text);
        assert!(!text.contains("Installed ·"));
        let install = action(list.upcast_ref(), "Arramooz Alwaseet", "Download").unwrap();
        let cache = crate::ipc::database_path()
            .parent()
            .unwrap()
            .join("packs/arramooz");
        std::fs::create_dir_all(&cache).unwrap();
        for extension in ["ifo", "idx", "dict"] {
            std::fs::copy(
                directory.join(format!("fixture.{extension}")),
                cache.join(format!("arramooz.{extension}")),
            )
            .unwrap();
        }
        std::fs::write(cache.join("ready.json"), "{}").unwrap();
        dictionaries::watch_installs(&list, &tx, &message);
        settle();
        install.grab_focus();
        settle();
        let offset = install.compute_bounds(&list).unwrap().y();
        position.set_value(f64::from(offset - 100.));
        settle();
        let download_position = position.value();
        assert!(
            download_position > 0.,
            "Exercise Download after scrolling the compact list"
        );
        install.emit_clicked();
        assert!(!install.is_sensitive());
        for _ in 0..50 {
            settle();
            if message.text().contains("installed:") {
                break;
            }
        }
        assert!(
            message.text().contains("installed: 1 entries"),
            "{}",
            message.text()
        );
        settle();
        assert!(
            (position.value() - download_position).abs() < 2.,
            "Downloading must retain the dictionary scroll position: before {download_position}, after {}",
            position.value()
        );
        let database =
            writy_core::dictionary::Dictionary::open(crate::ipc::database_path()).unwrap();
        assert_eq!(
            database.lookup("كتاب").unwrap()[0].source.name,
            "Arramooz Alwaseet"
        );
        settle();
        let mut switches = Vec::new();
        collect_switches(list.upcast_ref(), &mut switches);
        assert_eq!(switches.len(), 1);
        switches[0].set_active(false);
        settle();
        assert!(!database.source_states().unwrap()[0].enabled);
        assert!(database.lookup("كتاب").unwrap().is_empty());
        switches[0].set_active(true);
        settle();
        assert!(database.source_states().unwrap()[0].enabled);
        assert_eq!(database.lookup("كتاب").unwrap().len(), 1);
        std::fs::write(cache.join("arramooz.ifo"), "invalid header").unwrap();
        dictionaries::install_pack(&install, &dictionaries::CATALOGUE[5], &list, &tx, &message);
        for _ in 0..50 {
            settle();
            if message.text().starts_with("Installation failed:") {
                break;
            }
        }
        assert!(install.is_sensitive());
        assert!(message.text().starts_with("Installation failed:"));
        assert_eq!(
            database.lookup("كتاب").unwrap().len(),
            1,
            "Failed imports must keep the existing dictionary"
        );
        settle();
        assert!(action(list.upcast_ref(), "Arramooz Alwaseet", "Download").is_none());
        let remove = action(list.upcast_ref(), "Arramooz Alwaseet", "Remove").unwrap();
        remove.emit_clicked();
        assert!(!remove.is_sensitive());
        // A second action on the same dictionary must not start a download.
        let competing = gtk::Button::with_label("Download");
        dictionaries::install_pack(
            &competing,
            &dictionaries::CATALOGUE[5],
            &list,
            &tx,
            &message,
        );
        assert!(competing.is_sensitive());
        assert!(message.text().starts_with("Removing"));
        for _ in 0..50 {
            settle();
            if message.text().contains("removed.") {
                break;
            }
        }
        assert!(message.text().contains("removed."), "{}", message.text());
        assert!(database.lookup("كتاب").unwrap().is_empty());
        assert!(database.source_states().unwrap().is_empty());
        assert!(!cache.exists());
        assert!(path.is_file(), "Original import files must survive removal");
        settle();
        assert!(action(list.upcast_ref(), "Arramooz Alwaseet", "Remove").is_none());
        let download = action(list.upcast_ref(), "Arramooz Alwaseet", "Download").unwrap();
        assert!(download.is_sensitive());
        std::fs::create_dir_all(&cache).unwrap();
        for extension in ["ifo", "idx", "dict"] {
            std::fs::copy(
                directory.join(format!("fixture.{extension}")),
                cache.join(format!("arramooz.{extension}")),
            )
            .unwrap();
        }
        std::fs::write(cache.join("ready.json"), "{}").unwrap();
        download.emit_clicked();
        for _ in 0..50 {
            settle();
            if message.text().contains("installed:") {
                break;
            }
        }
        assert!(
            message.text().contains("installed: 1 entries"),
            "{}",
            message.text()
        );
        assert!(database.source_states().unwrap()[0].enabled);
        settle();
        // A broken cache must not remove a still usable installed dictionary.
        std::fs::remove_dir_all(&cache).unwrap();
        std::fs::write(&cache, "not a directory").unwrap();
        action(list.upcast_ref(), "Arramooz Alwaseet", "Remove")
            .unwrap()
            .emit_clicked();
        for _ in 0..50 {
            settle();
            if message.text().starts_with("Removal failed:") {
                break;
            }
        }
        assert!(
            message.text().starts_with("Removal failed:"),
            "{}",
            message.text()
        );
        assert_eq!(database.lookup("كتاب").unwrap().len(), 1);
        settle();
        assert!(
            action(list.upcast_ref(), "Arramooz Alwaseet", "Remove")
                .unwrap()
                .is_sensitive()
        );
        std::fs::remove_file(&cache).unwrap();
        preview.close();
    }
    // All floating components use the same synthetic dictionary fixture.
    search.set_visible(false);
    view.borrow_mut().show(Results {
        writing: true,
        ..result.clone()
    });
    // Stress the real reading surface with long, mixed-script content.
    result.entries[0].senses[0].examples = vec![
        "A good decision takes context into account. This example remains fully readable, including its final sentence.".repeat(10),
        "قَرَارٌ واضح يساعد الكاتب على اختيار الكلمات المناسبة، مع الحفاظ على النص العربي كاملاً.".repeat(6),
    ];
    view.borrow_mut().show(Results {
        writing: true,
        ..result.clone()
    });
    let hud_window = view.borrow().hud.window.clone();
    hud_window.set_resizable(false);
    snapshot(&hud_window, "hud-reading-top");
    {
        let current = view.borrow();
        let section = &current.hud.sections["translation"];
        ui_contrast.insert(
            "hud_source",
            contrast(section.source.upcast_ref(), settings_background),
        );
        ui_contrast.insert(
            "hud_body",
            contrast(section.body.upcast_ref(), settings_background),
        );
    }
    for (name, ratio) in &ui_contrast {
        assert!(
            *ratio >= 4.5,
            "{name} contrast {ratio} must be at least 4.5:1"
        );
    }
    let directory = std::env::var("WRITY_UI_SNAPSHOTS").unwrap_or_else(|_| "/tmp/writy-ui".into());
    std::fs::write(
        format!("{directory}/ui-contrast.json"),
        serde_json::to_string_pretty(&ui_contrast).unwrap(),
    )
    .unwrap();
    assert_eq!(view.borrow().hud.tabs.n_pages(), 7);
    assert_eq!(
        view.borrow().hud.tabs.current_page(),
        Some(0),
        "Translation is the default HUD tab"
    );
    for (index, (key, _)) in CATEGORIES.iter().enumerate() {
        view.borrow().hud.tabs.set_current_page(Some(index as u32));
        settle();
        let view = view.borrow();
        let section = &view.hud.sections[key];
        assert!(section.card.is_visible());
        assert!(section.body.is_selectable());
        let arabic = section.body.text().contains("قَرَار");
        if arabic {
            assert_eq!(section.body.xalign(), 0.);
            assert_eq!(section.body.direction(), gtk::TextDirection::Rtl);
        }
        assert!(section.body.wraps(), "HUD content must wrap");
        assert_eq!(section.body.ellipsize(), gtk::pango::EllipsizeMode::None);
        if *key != "suggestions" {
            assert!(
                section.body.height() >= section.body.layout().pixel_size().1,
                "body allocation must include every line"
            );
        }
    }
    {
        let tabs = view.borrow().hud.tabs.clone();
        tabs.set_current_page(Some(0));
        scroll_tab(&tabs, 1.);
        assert_eq!(
            tabs.current_page(),
            Some(1),
            "The mouse wheel over a HUD tab must select the next section"
        );
        tabs.set_current_page(Some(0));
        hud::switch_section(&tabs, -1.);
        assert_eq!(tabs.current_page(), Some(6));
        hud::switch_section(&tabs, 1.);
        assert_eq!(tabs.current_page(), Some(0));
        view.borrow().hud.sections["suggestions"]
            .scroll
            .set_visible(false);
        hud::switch_section(&tabs, 1.);
        assert_eq!(tabs.current_page(), Some(2), "Skip disabled sections");
        view.borrow().hud.sections["suggestions"]
            .scroll
            .set_visible(true);
    }
    if layer_shell_supported() {
        let surface = hud_window.child().unwrap();
        let header = surface.first_child().unwrap();
        let controllers = header.observe_controllers();
        let drag = (0..controllers.n_items())
            .find_map(|i| controllers.item(i).and_downcast::<gtk::GestureDrag>())
            .unwrap();
        let before = (hud_window.margin(Edge::Left), hud_window.margin(Edge::Top));
        drag.emit_by_name::<()>("drag-begin", &[&20f64, &20f64]);
        drag.emit_by_name::<()>("drag-update", &[&-40f64, &24f64]);
        settle();
        let moved = (hud_window.margin(Edge::Left), hud_window.margin(Edge::Top));
        assert_ne!(before, moved, "Header dragging must move the panel");
        view.borrow_mut().show(Results {
            writing: true,
            ..result.clone()
        });
        assert_eq!(
            moved,
            (hud_window.margin(Edge::Left), hud_window.margin(Edge::Top)),
            "Refresh must preserve manual placement"
        );
        snapshot(&hud_window, "hud-dragged");
    }
    view.borrow().hud.tabs.set_current_page(Some(4));
    settle();
    snapshot(&hud_window, "hud-examples-tab");
    {
        let view = view.borrow();
        let body = &view.hud.sections["examples"].body;
        body.select_region(0, 8);
        assert_eq!(body.selection_bounds(), Some((0, 8)));
        if layer_shell_supported() {
            assert_eq!(view.hud.window.keyboard_mode(), KeyboardMode::None);
        }
    }
    view.borrow_mut().show(Results {
        writing: true,
        ..result.clone()
    });
    assert_eq!(
        view.borrow().hud.sections["examples"]
            .body
            .selection_bounds(),
        Some((0, 8))
    );
    let fixed_height = hud_window.height();
    let adjustment = view.borrow().hud.scroll().vadjustment();
    assert!(
        adjustment.upper() > adjustment.page_size(),
        "long content needs scrolling"
    );
    adjustment.set_value(adjustment.upper() - adjustment.page_size());
    snapshot(&hud_window, "hud-reading-bottom");
    assert!(adjustment.value() > 0.);
    assert_eq!(
        hud_window.height(),
        fixed_height,
        "Scrolling must not resize the panel"
    );
    let previous_scroll = adjustment.value();
    view.borrow_mut().show(Results {
        writing: true,
        ..result.clone()
    });
    settle();
    assert!(
        (adjustment.value() - previous_scroll).abs() < 1.,
        "same-query updates must preserve reading position: {} -> {} (upper {}, page {})",
        previous_scroll,
        adjustment.value(),
        adjustment.upper(),
        adjustment.page_size()
    );
    view.borrow().hud.expand.set_active(true);
    snapshot(&hud_window, "hud-reading-expanded");
    if layer_shell_supported() {
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            assert!(hud_window.is_anchor(edge));
            assert_eq!(hud_window.margin(edge), 0);
        }
        let bounds = view.borrow().layout_bounds.unwrap();
        assert_eq!(
            (hud_window.width(), hud_window.height()),
            (bounds.2, bounds.3),
            "Fullscreen must fill the entire monitor"
        );
    }
    view.borrow().hud.expand.set_active(false);
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_enable_animations(false);
        let section = &view.borrow().hud.sections["translation"];
        let icons = section
            .copy
            .child()
            .unwrap()
            .downcast::<gtk::Stack>()
            .unwrap();
        assert_eq!(icons.transition_type(), gtk::StackTransitionType::None);
        section.copy.emit_clicked();
        assert_eq!(icons.visible_child_name().as_deref(), Some("copied"));
        assert!(hud_window.has_css_class("writy-reduced-motion"));
        settings.set_gtk_enable_animations(true);
    }
    // Reading remains reachable at a narrow monitor width and larger desktop text.
    view.borrow_mut().show(Results {
        writing: true,
        output_bounds: Some((0, 0, 360, 700)),
        ..result.clone()
    });
    adjustment.set_value(0.);
    snapshot(&hud_window, "hud-reading-narrow");
    assert!(hud_window.width() <= 360);
    assert!(hud_window.height() <= 700);
    if let Some(settings) = gtk::Settings::default() {
        let font = settings.gtk_font_name();
        settings.set_gtk_font_name(Some("Sans 20"));
        view.borrow_mut().show(Results {
            writing: true,
            output_bounds: Some((0, 0, 360, 700)),
            ..result.clone()
        });
        snapshot(&hud_window, "hud-reading-large-text");
        let borrowed = view.borrow();
        let section = &borrowed.hud.sections["examples"];
        assert!(section.body.height() >= section.body.layout().pixel_size().1);
        drop(borrowed);
        settings.set_gtk_font_name(font.as_deref());
        settings.set_gtk_theme_name(Some("Adwaita-dark"));
        view.borrow_mut().show(Results {
            writing: true,
            ..result.clone()
        });
        snapshot(&hud_window, "hud-reading-dark");
    }
    hud_window.set_visible(false);
    search.present();
    view.borrow_mut().show(result.clone());
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_theme_name(Some("Adwaita-dark"));
    }
    snapshot(&search, "search-dark");
    // Small window and larger text expose clipping in controls and labels.
    resize(&search, 360, 720);
    snapshot(&search, "search-narrow");
    assert!(
        search.width() <= 400,
        "narrow layout was not exercised: {}",
        search.width()
    );
    assert!(
        tabs.measure(gtk::Orientation::Horizontal, -1).0 <= view.borrow().content.width(),
        "Result tabs must fit the narrow reading surface: minimum {}, allocated {}",
        tabs.measure(gtk::Orientation::Horizontal, -1).0,
        view.borrow().content.width()
    );
    view.borrow_mut().show(Results {
        entries: vec![],
        status: "Generating…".into(),
        ..result.clone()
    });
    assert!(!view.borrow().copy.is_sensitive());
    assert!(view.borrow().activity.is_spinning());
    snapshot(&search, "search-loading");
    view.borrow_mut().show(Results {
        entries: vec![],
        status: "Dictionary unavailable. Check Settings.".into(),
        ..result
    });
    assert!(!view.borrow().activity.is_spinning());
    snapshot(&search, "search-empty-result");
    search.set_visible(false);
    let mut selection = view.borrow().last_result.clone().unwrap();
    selection.snapshot.source = "selection".into();
    selection.snapshot.text = "I will attend the meeting tomorrow.\nPlease send the agenda.".into();
    selection.entries.clear();
    selection.translation = None;
    selection.status = "Generating…".into();
    view.borrow().hud.tabs.set_current_page(Some(1));
    view.borrow_mut().show(selection.clone());
    assert!(hud_window.is_visible());
    assert_eq!(
        view.borrow().hud.tabs.current_page(),
        Some(1),
        "Suggestions remains selected for a new phrase"
    );
    view.borrow().hud.tabs.set_current_page(Some(0));
    assert_eq!(
        view.borrow().hud.sections["translation"].body.text(),
        "Translating selected text…"
    );
    snapshot(&hud_window, "selected-passage-loading");
    selection.translation = Some((
        "سأحضر الاجتماع غدًا.\nيرجى إرسال جدول الأعمال.".into(),
        "Fixture model".into(),
    ));
    selection.status.clear();
    view.borrow_mut().show(selection);
    assert!(hud_window.is_visible());
    assert!(
        view.borrow().hud.sections["translation"]
            .body
            .text()
            .contains("جدول الأعمال")
    );
    snapshot(&hud_window, "selected-passage-translated");
    for window in app.windows() {
        window.close();
    }
}

fn arabic_result_reading_layout() {
    gtk::init().unwrap();
    let body = label(
        "لعب\nثلاثي\nلازم\nيتصرف في: الماضي، المضارع، الأمر، المؤكد",
        "writy-body",
    );
    body.set_selectable(true);
    let window = gtk::Window::builder()
        .default_width(900)
        .default_height(400)
        .child(&body)
        .build();
    window.present();
    settle();
    let layout = body.layout();
    let (offset, _) = body.layout_offsets();
    let first = layout.index_to_pos(0).x() / gtk::pango::SCALE + offset;
    assert!(
        first > body.width() * 3 / 4,
        "Arabic must begin at the physical right inset, not in the middle or left"
    );
    window.close();
}

#[test]
#[ignore = "requires a Wayland compositor with layer-shell; synthetic content only"]
fn native_hud_drag_preserves_pointer_position() {
    gtk::init().unwrap();
    assert!(layer_shell_supported());
    let app = gtk::Application::builder()
        .application_id("io.github.lodaq.writy.drag-test")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let (tx, _rx) = tokio::sync::mpsc::channel(128);
    let view = View::new(&app, tx, Config::default());
    view.borrow_mut().show(Results {
        writing: true,
        output_bounds: None,
        generation: 1,
        snapshot: TextSnapshot {
            source: "synthetic".into(),
            focus_epoch: 1,
            revision: 1,
            language: Language::English,
            text: "drag fixture".into(),
            confidence: Confidence::Projected,
        },
        entries: vec![],
        suggestions: vec![],
        translation: Some(("اختبار".into(), "synthetic".into())),
        status: String::new(),
        caret: None,
        output: None,
    });
    let window = view.borrow().hud.window.clone();
    window.present();
    settle();
    let header = window.child().unwrap().first_child().unwrap();
    let controllers = header.observe_controllers();
    let drag = (0..controllers.n_items())
        .find_map(|i| controllers.item(i).and_downcast::<gtk::GestureDrag>())
        .unwrap();
    let initial = (window.margin(Edge::Left), window.margin(Edge::Top));
    drag.emit_by_name::<()>("drag-begin", &[&20f64, &20f64]);
    drag.emit_by_name::<()>("drag-update", &[&-10f64, &4f64]);
    drag.emit_by_name::<()>("drag-update", &[&-40f64, &24f64]);
    assert_eq!(
        (window.margin(Edge::Left), window.margin(Edge::Top)),
        initial,
        "Pointer events must coalesce until the next display frame"
    );
    settle();
    let moved = (window.margin(Edge::Left), window.margin(Edge::Top));
    assert_ne!(moved, initial);
    // The popup caught up with the pointer: local coordinates return to the grab point.
    drag.emit_by_name::<()>("drag-update", &[&0f64, &0f64]);
    settle();
    assert_eq!(
        (window.margin(Edge::Left), window.margin(Edge::Top)),
        moved,
        "Moving the surface must not make the next drag event pull it back"
    );
    drag.emit_by_name::<()>("drag-end", &[&0f64, &0f64]);
    window.close();
}

#[test]
#[ignore = "requires an isolated graphical session"]
fn native_bilingual_tabs_preserve_selection_without_translate_button() {
    gtk::init().unwrap();
    let app = gtk::Application::builder()
        .application_id("org.example.BilingualTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(128);
    let view = View::new(&app, tx, Config::default());
    let mut result = Results {
        generation: 1,
        snapshot: TextSnapshot {
            source: "search".into(),
            focus_epoch: 1,
            revision: 1,
            language: Language::English,
            text: "play".into(),
            confidence: Confidence::Projected,
        },
        entries: vec![LexicalEntry {
            headword: "play".into(),
            senses: vec![Sense {
                definitions: vec!["A game.".into()],
                ..Default::default()
            }],
            ..Default::default()
        }],
        suggestions: vec![],
        translation: Some(("لعب".into(), "fixture".into())),
        status: String::new(),
        writing: false,
        caret: None,
        output: None,
        output_bounds: None,
    };
    let search = view.borrow().search.clone();
    search.present();
    view.borrow_mut().show(result.clone());
    let tabs = view.borrow().results.clone();
    let title = |tabs: &gtk::Notebook| {
        tabs.tab_label_text(&tabs.nth_page(tabs.current_page()).unwrap())
            .unwrap()
            .to_string()
    };
    assert_eq!(title(&tabs), "Translation");
    tabs.set_current_page(Some(2));
    assert_eq!(title(&tabs), "Definitions");
    result.entries.clear();
    result.translation = None;
    result.status = "Generating…".into();
    result.snapshot.language = Language::Arabic;
    result.snapshot.text = "يلعب".into();
    view.borrow_mut().show(result.clone());
    assert_eq!(title(&tabs), "Translation");
    result.entries = vec![LexicalEntry {
        headword: "يلعب".into(),
        senses: vec![Sense {
            definitions: vec!["to play".into()],
            ..Default::default()
        }],
        ..Default::default()
    }];
    result.translation = Some(("play".into(), "fixture".into()));
    result.status.clear();
    view.borrow_mut().show(result.clone());
    assert_eq!(
        title(&tabs),
        "Definitions",
        "restore the selected tab after pending results"
    );
    tabs.set_current_page(Some(0));
    view.borrow_mut().show(result.clone());
    assert_eq!(title(&tabs), "Translation");
    assert_eq!(*view.borrow().search_copy_text.borrow(), "play");
    let mut pending = vec![search.clone().upcast::<gtk::Widget>()];
    while let Some(widget) = pending.pop() {
        if let Some(button) = widget.downcast_ref::<gtk::Button>() {
            assert_ne!(button.label().as_deref(), Some("Translate"));
            assert_ne!(button.label().as_deref(), Some("Look up or improve"));
            if button.label().as_deref() == Some("Copy result") {
                assert!(!button.is_visible());
            }
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            pending.push(widget);
        }
    }
    result.snapshot.text = "This is bad.".into();
    result.snapshot.language = Language::English;
    result.entries.clear();
    result.suggestions = ["better", "clearer"]
        .into_iter()
        .map(|replacement| writy_core::types::Suggestion {
            snapshot: result.snapshot.clone(),
            start: 8,
            end: 11,
            replacement: replacement.into(),
            description: "Alternative".into(),
            kind: SuggestionKind::Rewrite,
            provider: "fixture".into(),
        })
        .collect();
    view.borrow().editor.buffer().set_text(" This is bad. ");
    view.borrow_mut().show(result);
    tabs.set_current_page(Some(1));
    assert_eq!(title(&tabs), "Suggestions");
    let mut pending = vec![tabs.nth_page(Some(1)).unwrap()];
    let mut apply = Vec::new();
    let mut dismiss = Vec::new();
    while let Some(widget) = pending.pop() {
        if let Some(button) = widget.downcast_ref::<gtk::Button>()
            && button.label().as_deref() == Some("Apply")
        {
            apply.push(button.clone());
        }
        if let Some(button) = widget.downcast_ref::<gtk::Button>()
            && button.label().as_deref() == Some("Dismiss")
        {
            dismiss.push(button.clone());
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            pending.push(widget);
        }
    }
    assert_eq!(apply.len(), 2);
    assert!(
        apply.iter().all(|button| button.is_focusable()),
        "search choices are keyboard accessible"
    );
    apply[0].emit_clicked();
    let buffer = view.borrow().editor.buffer();
    let applied = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
    assert!(applied.trim() == "This is better." || applied.trim() == "This is clearer.");
    assert!(
        applied.starts_with(' ') && applied.ends_with(' '),
        "Apply preserves input padding"
    );
    apply[1].emit_clicked();
    assert_eq!(
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), false),
        applied,
        "stale choices cannot overwrite edits"
    );
    assert!(!apply[1].is_sensitive());
    assert_eq!(dismiss.len(), 2);
    assert!(dismiss.iter().all(|button| button.is_focusable()));
    while rx.try_recv().is_ok() {}
    dismiss[0].emit_clicked();
    assert!(matches!(
        rx.try_recv(),
        Ok((Command::DismissSuggestion { .. }, _))
    ));
    assert_eq!(
        buffer.text(&buffer.start_iter(), &buffer.end_iter(), false),
        applied,
        "Dismiss must not edit text"
    );
    snapshot(&search, "automatic-arabic-translation");
    search.close();
}

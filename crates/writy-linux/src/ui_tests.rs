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

#[test]
#[ignore = "requires a graphical session; renders synthetic data only"]
fn native_component_gallery() {
    gtk::init().unwrap();
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
    let (tx, _rx) = tokio::sync::mpsc::channel(128);
    let view = View::new(&app, tx, Config::default());
    let search = view.borrow().search.clone();
    search.set_resizable(false);
    search.present();
    snapshot(&search, "search-empty");
    assert!(!view.borrow().copy.is_sensitive());
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
    assert!(tabs.n_pages() >= 6);
    let page = tabs
        .nth_page(Some(0))
        .unwrap()
        .downcast::<gtk::ScrolledWindow>()
        .unwrap();
    let viewport = page.child().unwrap().downcast::<gtk::Viewport>().unwrap();
    let card = viewport.child().unwrap().downcast::<gtk::Box>().unwrap();
    let body = card
        .first_child()
        .unwrap()
        .next_sibling()
        .unwrap()
        .downcast::<gtk::Label>()
        .unwrap();
    assert!(body.wraps());
    assert_eq!(body.ellipsize(), gtk::pango::EllipsizeMode::None);
    let header = card.first_child().unwrap().downcast::<gtk::Box>().unwrap();
    let copy = header
        .last_child()
        .unwrap()
        .downcast::<gtk::Button>()
        .unwrap();
    copy.emit_clicked();
    assert_eq!(copy.label().as_deref(), Some("Copied"));
    let definition = (0..tabs.n_pages())
        .find(|index| {
            tabs.tab_label_text(&tabs.nth_page(Some(*index)).unwrap())
                .as_deref()
                == Some("Definition")
        })
        .unwrap();
    tabs.set_current_page(Some(definition));
    assert!(view.borrow().search_copy_text.borrow().contains("choice"));
    view.borrow_mut().show(result.clone());
    assert_eq!(tabs.current_page(), Some(definition));
    snapshot(&search, "search-definition-tab");
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
    snapshot(&settings[0], "settings");
    let shell = settings[0].child().unwrap().downcast::<gtk::Box>().unwrap();
    let scroll = shell
        .first_child()
        .unwrap()
        .downcast::<gtk::ScrolledWindow>()
        .unwrap();
    let adjustment = scroll.vadjustment();
    adjustment.set_value(adjustment.upper() - adjustment.page_size());
    snapshot(&settings[0], "settings-panels");
    settings[0].set_visible(false);
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
    assert_eq!(view.borrow().hud.tabs.n_pages(), 7);
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
        assert!(
            section.body.height() >= section.body.layout().pixel_size().1,
            "body allocation must include every line"
        );
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
    let adjustment = view.borrow().hud.scroll.vadjustment();
    assert!(
        adjustment.upper() > adjustment.page_size(),
        "long content needs scrolling"
    );
    adjustment.set_value(adjustment.upper() - adjustment.page_size());
    snapshot(&hud_window, "hud-reading-bottom");
    assert!(adjustment.value() > 0.);
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
    search.set_default_size(360, 720);
    snapshot(&search, "search-narrow");
    assert!(
        search.width() <= 400,
        "narrow layout was not exercised: {}",
        search.width()
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
    for window in app.windows() {
        window.close();
    }
}

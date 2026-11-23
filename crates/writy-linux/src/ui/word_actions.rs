//! Word-only shortcuts shared by search and the floating panel.
use super::*;

pub(super) fn update(
    container: &gtk::Box,
    result: Option<&Results>,
    tx: &Commands,
    insights_enabled: bool,
) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
    let Some(result) = result.filter(|r| writy_core::types::is_single_word(&r.snapshot.text))
    else {
        container.set_visible(false);
        return;
    };
    container.set_visible(true);
    let word = result.snapshot.text.trim();
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    if result.snapshot.language == writy_core::types::Language::English {
        for (caption, voice) in [("US 🔊", "en-us"), ("UK 🔊", "en-gb")] {
            let button = components::action_button(caption);
            button.set_tooltip_text(Some(&format!(
                "Play {caption} pronunciation (requires espeak-ng or espeak)"
            )));
            let word = word.to_owned();
            button.connect_clicked(move |button| {
                let mut error = String::new();
                for program in ["espeak-ng", "espeak"] {
                    match std::process::Command::new(program)
                        .args(["-v", voice, "--", &word])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                    {
                        Ok(mut child) => {
                            std::thread::spawn(move || {
                                let _ = child.wait();
                            });
                            return;
                        }
                        Err(e) => error = e.to_string(),
                    }
                }
                button.set_tooltip_text(Some(&format!(
                    "Cannot play pronunciation: install espeak-ng. {error}"
                )));
                button.set_label("Audio unavailable");
            });
            row.append(&button);
        }
    }
    let encoded = glib::uri_escape_string(word, None::<&str>, false);
    let english = result.snapshot.language == writy_core::types::Language::English;
    let video_url = if english {
        format!("https://youglish.com/pronounce/{encoded}/english")
    } else {
        format!("https://www.youtube.com/results?search_query={encoded}")
    };
    let wiki_language = if english { "en" } else { "ar" };
    let video = link("Video", "View videos containing this word", &video_url);
    video.add_css_class("writy-word-video");
    row.append(&video);
    if insights_enabled {
        let insights = components::action_button("Insights");
        insights.add_css_class("writy-word-insights");
        insights.set_tooltip_text(Some("AI insights explain this word with definitions, examples and common combinations in the result tabs"));
        insights.set_sensitive(result.status != "Generating…");
        let tx = tx.clone();
        let generation = result.generation;
        insights.connect_clicked(move |_| send(&tx, Command::WordInsights { generation }));
        row.append(&insights);
    }
    container.append(&row);
    let searches = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    for (caption, tooltip, url) in [
        (
            "Images",
            "Search Google Images for this word",
            format!("https://www.google.com/search?tbm=isch&q={encoded}"),
        ),
        (
            "Google",
            "Search Google for this word",
            format!("https://www.google.com/search?q={encoded}"),
        ),
        (
            "Wikipedia",
            "Search Wikipedia for this word",
            format!("https://{wiki_language}.wikipedia.org/w/index.php?search={encoded}"),
        ),
    ] {
        searches.append(&link(caption, tooltip, &url));
    }
    container.append(&searches);
}

fn link(caption: &str, tooltip: &str, uri: &str) -> gtk::LinkButton {
    let button = gtk::LinkButton::with_label(uri, caption);
    button.set_tooltip_text(Some(tooltip));
    button
}

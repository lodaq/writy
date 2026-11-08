//! Reading-result meaning shared by search and HUD rendering.
use crate::protocol::Results;
use std::collections::BTreeMap;
use writy_core::types::SuggestionKind;

pub(crate) const CATEGORIES: [(&str, &str); 7] = [
    ("translation", "Translation"),
    ("suggestions", "Writing"),
    ("definition", "Definition"),
    ("collocations", "Collocations"),
    ("examples", "Examples"),
    ("synonyms", "Synonyms & antonyms"),
    ("word_family", "Word family"),
];
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResultRow {
    pub(crate) headword: String,
    pub(crate) part_of_speech: String,
    pub(crate) text: String,
    pub(crate) markup: Option<String>,
    search_text: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SourceGroup {
    pub(crate) source: String,
    pub(crate) rows: Vec<ResultRow>,
}
pub(crate) struct Panel {
    pub(crate) category: &'static str,
    pub(crate) title: &'static str,
    pub(crate) body: String,
    pub(crate) groups: Vec<SourceGroup>,
    pub(crate) copy_text: String,
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
pub(crate) fn panels(result: &Results) -> Vec<Panel> {
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
                search_text: None,
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
                            search_text: None,
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
                        search_text: None,
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
                search_text: None,
            },
        );
    }
    CATEGORIES
        .into_iter()
        .filter_map(|(key, title)| {
            let mut groups = content.remove(key).unwrap_or_default();
            if key == "definition" {
                for group in &mut groups {
                    if group.source == "Arramooz Alwaseet" {
                        for row in &mut group.rows {
                            row.search_text = Some(compact_arramooz(&row.text));
                        }
                    }
                }
            }
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

impl ResultRow {
    pub(crate) fn search_text(&self) -> &str {
        self.search_text.as_deref().unwrap_or(&self.text)
    }
    pub(crate) fn search_markup(&self) -> Option<&str> {
        if self.search_text.is_some() {
            None
        } else {
            self.markup.as_deref()
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum DisplayText {
    Plain(String),
    Markup(String),
}
impl SourceGroup {
    pub(crate) fn body(&self) -> DisplayText {
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
        assert_eq!(row.search_text(), "فعل [لعب]\nثلاثي · لازم");
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
            search_text: None,
        };
        let group = SourceGroup {
            source: "fixture".into(),
            rows: vec![row("bold", Some("<b>bold</b>")), row("x < y & z", None)],
        };
        assert_eq!(
            group.body(),
            DisplayText::Markup("<b>bold</b>\n\nx &lt; y &amp; z".into())
        );
        assert_eq!(group.rows[0].search_markup(), Some("<b>bold</b>"));
        let compact = ResultRow {
            search_text: Some("compact text".into()),
            ..row("raw text", Some("<b>raw text</b>"))
        };
        assert_eq!(compact.search_text(), "compact text");
        assert_eq!(compact.search_markup(), None);
        assert_eq!(compact.text, "raw text");
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

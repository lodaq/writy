use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    English,
    Arabic,
}

impl Language {
    pub fn detect(text: &str) -> Self {
        if text.chars().any(|c| matches!(c as u32, 0x0600..=0x06ff | 0x0750..=0x077f | 0x08a0..=0x08ff | 0xfb50..=0xfdff | 0xfe70..=0xfeff)) {
            Self::Arabic
        } else {
            Self::English
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Arabic => "ar",
        }
    }
    pub fn opposite(self) -> Self {
        match self {
            Self::English => Self::Arabic,
            Self::Arabic => Self::English,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Projected,
    Verified,
}

// Text-bearing types deliberately do not implement Debug: operational logs must not expose prose.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextSnapshot {
    pub source: String,
    pub focus_epoch: u64,
    pub revision: u64,
    pub language: Language,
    pub text: String,
    pub confidence: Confidence,
}
impl TextSnapshot {
    pub fn same_context(&self, other: &Self) -> bool {
        self.source == other.source
            && self.focus_epoch == other.focus_epoch
            && self.revision == other.revision
            && self.language == other.language
            && self.text == other.text
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionKind {
    Rewrite,
    Spelling,
    Grammar,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub snapshot: TextSnapshot,
    // Character offsets, never UTF-8 byte offsets.
    pub start: usize,
    pub end: usize,
    pub replacement: String,
    pub description: String,
    pub kind: SuggestionKind,
    pub provider: String,
}

impl Suggestion {
    pub fn same_edit(&self, other: &Self) -> bool {
        self.snapshot.same_context(&other.snapshot)
            && self.start == other.start
            && self.end == other.end
            && self.replacement == other.replacement
    }
    pub fn matches(&self, current: &TextSnapshot) -> bool {
        self.snapshot.same_context(current)
            && self.start <= self.end
            && self.end <= current.text.chars().count()
    }
    pub fn replaced_text(&self) -> Option<String> {
        if self.start > self.end || self.end > self.snapshot.text.chars().count() {
            return None;
        }
        Some(
            self.snapshot
                .text
                .chars()
                .take(self.start)
                .chain(self.replacement.chars())
                .chain(self.snapshot.text.chars().skip(self.end))
                .collect(),
        )
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Attribution {
    pub name: String,
    pub url: String,
    pub license: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Sense {
    pub part_of_speech: String,
    pub definitions: Vec<String>,
    pub translations: Vec<String>,
    pub examples: Vec<String>,
    pub collocations: Vec<String>,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
    pub word_family: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LexicalEntry {
    pub headword: String,
    pub language: Language,
    pub senses: Vec<Sense>,
    #[serde(default)]
    pub definition_markup: Option<String>,
    pub pronunciation: String,
    pub audio: Vec<String>,
    pub images: Vec<String>,
    pub source: Attribution,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    WritingAssistance,
    Improve,
    Rewrite,
    Translate,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ProviderResult {
    pub snapshot: TextSnapshot,
    pub capability: Capability,
    pub text: String,
    pub provider: String,
    pub notice: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_choice_keeps_its_edit_identity_after_reordering_and_rejects_changed_context() {
        let choice = Suggestion {
            snapshot: TextSnapshot {
                source: "editor".into(),
                focus_epoch: 1,
                revision: 1,
                language: Language::English,
                text: "bad".into(),
                confidence: Confidence::Projected,
            },
            start: 0,
            end: 3,
            replacement: "better".into(),
            description: "Alternative".into(),
            kind: SuggestionKind::Rewrite,
            provider: "fixture".into(),
        };
        let other = Suggestion {
            replacement: "clearer".into(),
            ..choice.clone()
        };
        let reordered = [other, choice.clone()];
        assert_eq!(
            reordered
                .iter()
                .find(|item| item.same_edit(&choice))
                .unwrap()
                .replacement,
            "better"
        );
        let changed = Suggestion {
            end: 2,
            ..choice.clone()
        };
        assert!(!changed.same_edit(&choice));
        let mut stale = choice.clone();
        stale.snapshot.revision += 1;
        assert!(!stale.same_edit(&choice));
    }
}

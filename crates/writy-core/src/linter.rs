use crate::types::{Language, Suggestion, SuggestionKind, TextSnapshot};
use harper_core::{
    Dialect, Document,
    linting::{LintGroup, Linter, Suggestion as Edit},
};

pub struct EnglishChecker {
    group: LintGroup,
}
impl Default for EnglishChecker {
    fn default() -> Self {
        Self {
            group: LintGroup::new_curated(
                harper_core::spell::FstDictionary::curated(),
                Dialect::American,
            ),
        }
    }
}
impl EnglishChecker {
    pub fn check(&mut self, snapshot: &TextSnapshot) -> Vec<Suggestion> {
        if snapshot.language != Language::English || snapshot.text.trim().is_empty() {
            return vec![];
        }
        let doc = Document::new_plain_english_curated(&snapshot.text);
        let mut lints = self.group.lint(&doc);
        lints.sort_by_key(|lint| lint.priority);
        lints
            .into_iter()
            .filter_map(|lint| {
                let edit = lint.suggestions.first()?;
                let (start, end, replacement) = match edit {
                    Edit::ReplaceWith(chars) => {
                        (lint.span.start, lint.span.end, chars.iter().collect())
                    }
                    Edit::InsertAfter(chars) => {
                        (lint.span.end, lint.span.end, chars.iter().collect())
                    }
                    Edit::Remove => (lint.span.start, lint.span.end, String::new()),
                };
                let kind = if lint.lint_kind == harper_core::linting::LintKind::Spelling {
                    SuggestionKind::Spelling
                } else {
                    SuggestionKind::Grammar
                };
                Some(Suggestion {
                    snapshot: snapshot.clone(),
                    start,
                    end,
                    replacement,
                    description: lint.message,
                    kind,
                    provider: "Harper · local".into(),
                })
            })
            .take(5)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::TypingBuffer;
    #[test]
    fn misspelling_has_applicable_character_range() {
        let mut b = TypingBuffer::default();
        b.push("I recieved the message.");
        let s = EnglishChecker::default().check(&b.snapshot());
        assert!(
            s.iter()
                .any(|s| s.replaced_text().is_some_and(|t| t.contains("received")))
        );
        assert!(s.iter().all(|s| s.matches(&b.snapshot())));
    }
    #[test]
    fn no_english_grammar_claim_for_arabic() {
        let mut b = TypingBuffer::default();
        b.push("هذه رسالة عربية");
        assert!(EnglishChecker::default().check(&b.snapshot()).is_empty());
    }
}

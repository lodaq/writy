use crate::{
    config::{EnglishDialect, HarperConfig},
    types::{Language, Suggestion, SuggestionKind, TextSnapshot},
};
use harper_core::{
    Dialect, DictWordMetadata, Document,
    linting::{Lint, LintGroup, LintKind, Suggestion as Edit},
    spell::{FstDictionary, MergedDictionary, MutableDictionary},
};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LintFinding {
    pub start: usize,
    pub end: usize,
    pub kind: SuggestionKind,
    pub category: String,
    pub rule: String,
    pub message: String,
    pub replacements: Vec<String>,
    pub provider: String,
}

pub struct EnglishChecker {
    group: LintGroup,
    dictionary: Arc<MergedDictionary>,
    config: HarperConfig,
}

impl Default for EnglishChecker {
    fn default() -> Self {
        Self::from_config(&HarperConfig::default())
    }
}

fn to_harper_dialect(dialect: EnglishDialect) -> Dialect {
    match dialect {
        EnglishDialect::American => Dialect::American,
        EnglishDialect::British => Dialect::British,
        EnglishDialect::Canadian => Dialect::Canadian,
        EnglishDialect::Australian => Dialect::Australian,
        EnglishDialect::Indian => Dialect::Indian,
    }
}

fn build_group(config: &HarperConfig) -> (LintGroup, Arc<MergedDictionary>) {
    let mut dict = MergedDictionary::new();
    dict.add_dictionary(FstDictionary::curated());
    if !config.dictionary.is_empty() {
        let mut user_dict = MutableDictionary::new();
        for word in &config.dictionary {
            let trimmed = word.trim();
            if !trimmed.is_empty() {
                user_dict.append_word_str(trimmed, DictWordMetadata::default());
            }
        }
        dict.add_dictionary(Arc::new(user_dict));
    }
    let dictionary = Arc::new(dict);
    let mut group = LintGroup::new_curated(dictionary.clone(), to_harper_dialect(config.dialect));
    for rule in &config.disabled_rules {
        group.config.set_rule_enabled(rule, false);
    }
    (group, dictionary)
}

fn classify_kind(lint_kind: LintKind) -> SuggestionKind {
    match lint_kind {
        LintKind::Spelling | LintKind::Typo => SuggestionKind::Spelling,
        LintKind::Enhancement
        | LintKind::Style
        | LintKind::Redundancy
        | LintKind::Regionalism
        | LintKind::Readability
        | LintKind::WordChoice
        | LintKind::Usage => SuggestionKind::Rewrite,
        _ => SuggestionKind::Grammar,
    }
}

fn edit_to_span_and_replacement(lint: &Lint, edit: &Edit) -> (usize, usize, String) {
    match edit {
        Edit::ReplaceWith(chars) => (lint.span.start, lint.span.end, chars.iter().collect()),
        Edit::InsertAfter(chars) => (lint.span.end, lint.span.end, chars.iter().collect()),
        Edit::Remove => (lint.span.start, lint.span.end, String::new()),
    }
}

impl EnglishChecker {
    pub fn from_config(config: &HarperConfig) -> Self {
        let (group, dictionary) = build_group(config);
        Self {
            group,
            dictionary,
            config: config.clone(),
        }
    }

    pub fn configure(&mut self, config: &HarperConfig) {
        if self.config.dialect == config.dialect
            && self.config.disabled_rules == config.disabled_rules
            && self.config.dictionary == config.dictionary
        {
            self.config.enabled = config.enabled;
            return;
        }
        let (group, dictionary) = build_group(config);
        self.config = config.clone();
        self.group = group;
        self.dictionary = dictionary;
    }

    pub fn check_findings(&mut self, text: &str) -> Vec<LintFinding> {
        if !self.config.enabled
            || text.trim().is_empty()
            || Language::detect(text) != Language::English
        {
            return vec![];
        }
        let doc = Document::new_plain_english(text, self.dictionary.as_ref());
        let organized = self.group.organized_lints(&doc);
        let mut items: Vec<(String, Lint)> = organized
            .into_iter()
            .flat_map(|(rule, lints)| lints.into_iter().map(move |lint| (rule.clone(), lint)))
            .collect();
        items.sort_by_key(|(_, lint)| (lint.span.start, lint.priority, lint.span.end));

        items
            .into_iter()
            .map(|(rule, lint)| {
                let kind = classify_kind(lint.lint_kind);
                let category = lint.lint_kind.to_string();
                let mut replacements = Vec::new();
                let mut span = (lint.span.start, lint.span.end);
                for (index, edit) in lint.suggestions.iter().enumerate() {
                    let (s, e, text) = edit_to_span_and_replacement(&lint, edit);
                    if index == 0 {
                        span = (s, e);
                    }
                    if !replacements.contains(&text) {
                        replacements.push(text);
                    }
                }
                LintFinding {
                    start: span.0,
                    end: span.1,
                    kind,
                    category,
                    rule,
                    message: lint.message,
                    replacements,
                    provider: "Harper · local".into(),
                }
            })
            .collect()
    }

    pub fn check(&mut self, snapshot: &TextSnapshot) -> Vec<Suggestion> {
        if !self.config.enabled
            || snapshot.language != Language::English
            || snapshot.text.trim().is_empty()
        {
            return vec![];
        }
        let doc = Document::new_plain_english(&snapshot.text, self.dictionary.as_ref());
        let organized = self.group.organized_lints(&doc);
        let mut lints: Vec<Lint> = organized.into_values().flatten().collect();
        lints.sort_by_key(|lint| (lint.priority, lint.span.start));
        lints
            .into_iter()
            .filter_map(|lint| {
                let edit = lint.suggestions.first()?;
                let (start, end, replacement) = edit_to_span_and_replacement(&lint, edit);
                let kind = classify_kind(lint.lint_kind);
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
    #[test]
    fn findings_include_multiple_suggestions_rules_and_improvements() {
        let mut checker = EnglishChecker::default();
        let findings = checker.check_findings("Wether i know very unique things.");
        let wether = findings
            .iter()
            .find(|f| f.start == 0 && f.end == 6 && f.kind == SuggestionKind::Spelling)
            .expect("spelling finding for Wether");
        assert_eq!(wether.rule, "SpellCheck");
        assert_eq!(wether.category, "Spelling");
        assert!(wether.replacements.len() > 1);
        assert!(wether.replacements.iter().any(|r| r == "Whether"));

        let cap = findings
            .iter()
            .find(|f| f.start == 7 && f.end == 8)
            .expect("capitalization finding for i");
        assert_eq!(cap.kind, SuggestionKind::Grammar);
        assert_eq!(cap.replacements, vec!["I".to_string()]);

        let unique = findings
            .iter()
            .find(|f| f.replacements.iter().any(|r| r.contains("special")))
            .expect("improvement finding for very unique");
        assert_eq!(unique.kind, SuggestionKind::Rewrite);
    }
    #[test]
    fn custom_dictionary_and_disabled_rules_suppress_findings() {
        let mut config = HarperConfig::default();
        config.add_dictionary_word("Wether");
        let mut checker = EnglishChecker::from_config(&config);
        let findings = checker.check_findings("Wether i know this.");
        assert!(
            !findings
                .iter()
                .any(|f| f.rule == "SpellCheck" && f.start == 0),
            "dictionary word should suppress SpellCheck"
        );
        if let Some(cap_rule) = findings
            .iter()
            .find(|f| f.start == 7)
            .map(|f| f.rule.clone())
        {
            config.disable_rule(&cap_rule);
            checker.configure(&config);
            let after = checker.check_findings("Wether i know this.");
            assert!(!after.iter().any(|f| f.rule == cap_rule));
        }
    }
}

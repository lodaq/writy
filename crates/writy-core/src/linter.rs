use crate::{
    config::{EnglishDialect, HarperConfig},
    types::{Language, Suggestion, SuggestionKind, TextSnapshot},
};
use harper_core::{
    Dialect, DictWordMetadata, Document, IgnoredLints, LintContext,
    linting::{FlatConfig, Lint, LintGroup, LintKind, Suggestion as Edit},
    parsers::MarkdownOptions,
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
    pub ignore_hash: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarperRuleInfo {
    pub name: String,
    pub description: String,
    pub default_enabled: bool,
    pub enabled: bool,
}

pub struct EnglishChecker {
    group: LintGroup,
    dictionary: Arc<MergedDictionary>,
    active_dialect: Dialect,
    ignored: IgnoredLints,
    config: HarperConfig,
}

impl Default for EnglishChecker {
    fn default() -> Self {
        Self::from_config(&HarperConfig::default())
    }
}

fn to_harper_dialect(dialect: EnglishDialect) -> Dialect {
    match dialect {
        EnglishDialect::Auto | EnglishDialect::American => Dialect::American,
        EnglishDialect::British => Dialect::British,
        EnglishDialect::Canadian => Dialect::Canadian,
        EnglishDialect::Australian => Dialect::Australian,
        EnglishDialect::Indian => Dialect::Indian,
    }
}

fn build_dictionary(config: &HarperConfig) -> Arc<MergedDictionary> {
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
    Arc::new(dict)
}

fn build_group(
    config: &HarperConfig,
    dictionary: Arc<MergedDictionary>,
    dialect: Dialect,
) -> LintGroup {
    let mut group = LintGroup::new_curated(dictionary, dialect);
    for rule in &config.disabled_rules {
        group.config.set_rule_enabled(rule, false);
    }
    for (rule, enabled) in &config.rule_overrides {
        group.config.set_rule_enabled(rule, *enabled);
    }
    group
}

fn build_ignored(hashes: &[u64]) -> IgnoredLints {
    let mut ignored = IgnoredLints::new();
    for &hash in hashes {
        ignored.ignore_hash(hash);
    }
    ignored
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
        let dictionary = build_dictionary(config);
        let active_dialect = to_harper_dialect(config.dialect);
        let group = build_group(config, dictionary.clone(), active_dialect);
        let ignored = build_ignored(&config.ignored_hashes);
        Self {
            group,
            dictionary,
            active_dialect,
            ignored,
            config: config.clone(),
        }
    }

    pub fn configure(&mut self, config: &HarperConfig) {
        if self.config.ignored_hashes != config.ignored_hashes {
            self.ignored = build_ignored(&config.ignored_hashes);
        }
        let dict_or_rules_changed = self.config.dialect != config.dialect
            || self.config.disabled_rules != config.disabled_rules
            || self.config.rule_overrides != config.rule_overrides
            || self.config.dictionary != config.dictionary;
        if !dict_or_rules_changed {
            self.config = config.clone();
            return;
        }
        let dictionary = if self.config.dictionary != config.dictionary {
            build_dictionary(config)
        } else {
            self.dictionary.clone()
        };
        let active_dialect = if config.dialect == EnglishDialect::Auto {
            self.active_dialect
        } else {
            to_harper_dialect(config.dialect)
        };
        self.group = build_group(config, dictionary.clone(), active_dialect);
        self.dictionary = dictionary;
        self.active_dialect = active_dialect;
        self.config = config.clone();
    }

    pub fn rule_catalog(config: &HarperConfig) -> Vec<HarperRuleInfo> {
        let defaults = FlatConfig::new_curated();
        let group = LintGroup::new_curated(
            Arc::new(MutableDictionary::new()),
            to_harper_dialect(config.dialect),
        );
        let descriptions = group.all_descriptions();
        let mut effective = defaults.clone();
        for rule in &config.disabled_rules {
            effective.set_rule_enabled(rule, false);
        }
        for (rule, enabled) in &config.rule_overrides {
            effective.set_rule_enabled(rule, *enabled);
        }
        let mut catalog: Vec<HarperRuleInfo> = descriptions
            .into_iter()
            .map(|(name, desc)| HarperRuleInfo {
                name: name.to_owned(),
                description: desc.to_owned(),
                default_enabled: defaults.is_rule_enabled(name),
                enabled: effective.is_rule_enabled(name),
            })
            .collect();
        catalog.sort_by(|a, b| a.name.cmp(&b.name));
        catalog
    }

    fn parse_and_prepare(&mut self, text: &str) -> Document {
        let doc = if self.config.markdown {
            Document::new_markdown(text, MarkdownOptions::default(), self.dictionary.as_ref())
        } else {
            Document::new_plain_english(text, self.dictionary.as_ref())
        };
        if self.config.dialect == EnglishDialect::Auto {
            let guessed = Dialect::try_guess_from_document(&doc).unwrap_or(Dialect::American);
            if guessed != self.active_dialect {
                self.active_dialect = guessed;
                self.group = build_group(&self.config, self.dictionary.clone(), guessed);
            }
        }
        doc
    }

    pub fn check_findings(&mut self, text: &str) -> Vec<LintFinding> {
        if !self.config.enabled
            || text.trim().is_empty()
            || Language::detect(text) != Language::English
        {
            return vec![];
        }
        let doc = self.parse_and_prepare(text);
        let organized = self.group.organized_lints(&doc);
        let mut items: Vec<(String, Lint)> = organized
            .into_iter()
            .flat_map(|(rule, lints)| lints.into_iter().map(move |lint| (rule.clone(), lint)))
            .filter(|(_, lint)| !self.ignored.is_ignored(lint, &doc))
            .collect();
        items.sort_by_key(|(_, lint)| (lint.span.start, lint.priority, lint.span.end));

        items
            .into_iter()
            .map(|(rule, lint)| {
                let kind = classify_kind(lint.lint_kind);
                let category = lint.lint_kind.to_string();
                let ignore_hash = LintContext::from_lint(&lint, &doc).default_hash();
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
                    ignore_hash,
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
        let doc = self.parse_and_prepare(&snapshot.text);
        let organized = self.group.organized_lints(&doc);
        let mut items: Vec<(String, Lint)> = organized
            .into_iter()
            .flat_map(|(rule, lints)| lints.into_iter().map(move |lint| (rule.clone(), lint)))
            .filter(|(_, lint)| !self.ignored.is_ignored(lint, &doc))
            .collect();
        items.sort_by_key(|(_, lint)| (lint.priority, lint.span.start));
        items
            .into_iter()
            .filter_map(|(rule, lint)| {
                let first = lint.suggestions.first()?;
                let (start, end, replacement) = edit_to_span_and_replacement(&lint, first);
                let mut alternatives = Vec::new();
                for edit in &lint.suggestions {
                    let (_, _, text) = edit_to_span_and_replacement(&lint, edit);
                    if !alternatives.contains(&text) {
                        alternatives.push(text);
                    }
                }
                let kind = classify_kind(lint.lint_kind);
                let category = lint.lint_kind.to_string();
                let ignore_hash = LintContext::from_lint(&lint, &doc).default_hash();
                Some(Suggestion {
                    snapshot: snapshot.clone(),
                    start,
                    end,
                    replacement,
                    description: lint.message,
                    kind,
                    provider: "Harper · local".into(),
                    alternatives,
                    rule,
                    category,
                    ignore_hash: Some(ignore_hash),
                })
            })
            .take(50)
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
    #[test]
    fn markdown_ignored_hashes_rule_catalog_and_auto_dialect_work() {
        let mut config = HarperConfig {
            dialect: EnglishDialect::Auto,
            markdown: true,
            ..Default::default()
        };
        let mut checker = EnglishChecker::from_config(&config);
        // Inline code should be ignored when markdown mode is enabled.
        let md_findings = checker.check_findings("Use `recieved` in code, not recieved in prose.");
        let spell: Vec<_> = md_findings
            .iter()
            .filter(|f| f.rule == "SpellCheck")
            .collect();
        assert_eq!(spell.len(), 1);
        assert!(spell[0].start > 20);

        // Ignoring a finding by its Harper context hash suppresses that lint instance.
        config.add_ignored_hash(spell[0].ignore_hash);
        checker.configure(&config);
        let after_ignore = checker.check_findings("Use `recieved` in code, not recieved in prose.");
        assert!(
            !after_ignore.iter().any(|f| f.rule == "SpellCheck"),
            "ignored context hash should suppress matching lint"
        );

        // Rule catalog lists rules with descriptions and reflects overrides.
        config.set_rule_override("AvoidCurses", Some(true));
        let catalog = EnglishChecker::rule_catalog(&config);
        assert!(catalog.len() > 50);
        let spell_rule = catalog
            .iter()
            .find(|r| r.name == "SpellCheck")
            .expect("SpellCheck rule in catalog");
        assert!(spell_rule.default_enabled && spell_rule.enabled);
        assert!(!spell_rule.description.is_empty());

        // check() includes alternatives, rule, category, and ignore_hash.
        let mut buf = TypingBuffer::default();
        buf.push("Wether i know this.");
        let suggestions = EnglishChecker::default().check(&buf.snapshot());
        let wether = suggestions
            .iter()
            .find(|s| s.rule == "SpellCheck")
            .expect("SpellCheck suggestion");
        assert!(wether.choices().len() > 1);
        assert_eq!(wether.category, "Spelling");
        assert!(wether.ignore_hash.is_some());
    }
}

use crate::config::WordExpander;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionMatch {
    /// Number of Unicode characters to delete (trigger + postfix).
    pub delete_chars: usize,
    pub replacement: String,
}

/// Match a `trigger + postfix` at the end of `buffer_text` on a word boundary.
pub fn match_suffix(buffer_text: &str, config: &WordExpander) -> Option<ExpansionMatch> {
    if !config.enabled || config.postfix.is_empty() || config.rules.is_empty() {
        return None;
    }
    let without_postfix = buffer_text.strip_suffix(&config.postfix)?;
    let mut best: Option<(&str, &str)> = None;
    for rule in &config.rules {
        if rule.trigger.is_empty() {
            continue;
        }
        if let Some(prefix) = without_postfix.strip_suffix(&rule.trigger) {
            let boundary = prefix
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_alphanumeric() && c != '_');
            if boundary
                && best.is_none_or(|(prev_trigger, _)| rule.trigger.len() > prev_trigger.len())
            {
                best = Some((&rule.trigger, &rule.replacement));
            }
        }
    }
    let (trigger, replacement) = best?;
    Some(ExpansionMatch {
        delete_chars: trigger.chars().count() + config.postfix.chars().count(),
        replacement: replacement.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ExpansionRule;

    fn sample_config() -> WordExpander {
        WordExpander {
            enabled: true,
            postfix: ";".into(),
            rules: vec![
                ExpansionRule {
                    trigger: "idk".into(),
                    replacement: "i don't know".into(),
                },
                ExpansionRule {
                    trigger: "name".into(),
                    replacement: "khalid mesbah".into(),
                },
                ExpansionRule {
                    trigger: "ie".into(),
                    replacement: "for example".into(),
                },
            ],
        }
    }

    #[test]
    fn matches_standalone_or_word_boundary_with_postfix() {
        let cfg = sample_config();
        assert_eq!(
            match_suffix("idk;", &cfg),
            Some(ExpansionMatch {
                delete_chars: 4,
                replacement: "i don't know".into(),
            })
        );
        assert_eq!(
            match_suffix("hello name;", &cfg),
            Some(ExpansionMatch {
                delete_chars: 5,
                replacement: "khalid mesbah".into(),
            })
        );
        assert_eq!(
            match_suffix("(ie;", &cfg),
            Some(ExpansionMatch {
                delete_chars: 3,
                replacement: "for example".into(),
            })
        );
    }

    #[test]
    fn does_not_match_mid_word_or_without_postfix() {
        let cfg = sample_config();
        assert_eq!(match_suffix("idk", &cfg), None);
        assert_eq!(match_suffix("sidk;", &cfg), None);
        assert_eq!(match_suffix("friend;", &cfg), None);
        assert_eq!(match_suffix("ie; ", &cfg), None);
    }

    #[test]
    fn respects_disabled_flag_and_prefers_longest_trigger() {
        let mut cfg = sample_config();
        cfg.rules.push(ExpansionRule {
            trigger: "my_name".into(),
            replacement: "full name".into(),
        });
        assert_eq!(
            match_suffix("my_name;", &cfg),
            Some(ExpansionMatch {
                delete_chars: 8,
                replacement: "full name".into(),
            })
        );
        cfg.enabled = false;
        assert_eq!(match_suffix("idk;", &cfg), None);
    }
}

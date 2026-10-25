use crate::types::{Confidence, Language, TextSnapshot};

/// A projected segment is invalidated rather than guessed after uncertain edits.
#[derive(Default)]
pub struct TypingBuffer {
    source: String,
    focus_epoch: u64,
    revision: u64,
    text: String,
}

impl TypingBuffer {
    pub fn focus(&mut self, source: String, epoch: u64) {
        self.source = source;
        self.focus_epoch = epoch;
        self.reset();
    }
    pub fn reset(&mut self) {
        self.text.clear();
        self.revision = self.revision.wrapping_add(1);
    }
    pub fn push(&mut self, text: &str) {
        if text.chars().any(|c| c.is_control() && c != '\t') {
            self.reset();
            return;
        }
        if self.text.chars().count() + text.chars().count() > 2048 {
            self.reset();
        }
        self.text.extend(text.chars().take(2048));
        self.revision = self.revision.wrapping_add(1);
    }
    pub fn snapshot(&self) -> TextSnapshot {
        TextSnapshot {
            source: self.source.clone(),
            focus_epoch: self.focus_epoch,
            revision: self.revision,
            language: Language::detect(&self.text),
            text: self.text.clone(),
            confidence: Confidence::Projected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn focus_and_uncertain_edits_invalidate_arabic_suggestions() {
        let mut b = TypingBuffer::default();
        b.focus("editor".into(), 1);
        b.push("هذا نص");
        let old = b.snapshot();
        assert_eq!(old.language, Language::Arabic);
        b.reset();
        assert!(b.snapshot().text.is_empty());
        assert_ne!(old.revision, b.snapshot().revision);
        b.push("new text");
        b.focus("other".into(), 2);
        assert!(b.snapshot().text.is_empty());
        assert_eq!(b.snapshot().focus_epoch, 2);
    }
    #[test]
    fn replacement_offsets_are_unicode_characters() {
        use crate::types::{Suggestion, SuggestionKind};
        let mut b = TypingBuffer::default();
        b.push("هذا خطأ");
        let s = Suggestion {
            snapshot: b.snapshot(),
            start: 4,
            end: 7,
            replacement: "صحيح".into(),
            description: String::new(),
            kind: SuggestionKind::Rewrite,
            provider: "test".into(),
        };
        assert_eq!(s.replaced_text().as_deref(), Some("هذا صحيح"));
        b.push(".");
        assert!(!s.matches(&b.snapshot()));
    }
}

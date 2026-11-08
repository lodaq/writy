//! Request invalidation and the independent display-side stale-result check.
use crate::{accessibility::FieldSnapshot, input::Decoder, protocol::UiEvent};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use writy_core::{
    buffer::TypingBuffer,
    types::{LexicalEntry, Suggestion, TextSnapshot},
};

pub(crate) struct Generation {
    clock: Arc<AtomicU64>,
    pub buffer: TypingBuffer,
    pub decoder: Decoder,
    pub current: Option<TextSnapshot>,
    pub anchor: Option<(u64, FieldSnapshot)>,
    pub suggestions: Vec<Suggestion>,
    pub entries: Vec<LexicalEntry>,
    pub translation: Option<(String, String)>,
    pub notice: String,
    pub model: Option<tokio::task::JoinHandle<()>>,
    pub local_revision: u64,
    pub model_revision: u64,
    pub selection_position: Option<(i32, i32)>,
}
impl Generation {
    pub fn new(clock: Arc<AtomicU64>) -> Self {
        Self {
            clock,
            buffer: TypingBuffer::default(),
            decoder: Decoder::default(),
            current: None,
            anchor: None,
            suggestions: vec![],
            entries: vec![],
            translation: None,
            notice: String::new(),
            model: None,
            local_revision: u64::MAX,
            model_revision: u64::MAX,
            selection_position: None,
        }
    }
    pub fn id(&self) -> u64 {
        self.clock.load(Ordering::SeqCst)
    }
    pub fn cancel_model(&mut self) {
        if let Some(task) = self.model.take() {
            task.abort();
        }
    }
    fn clear_results(&mut self) {
        self.cancel_model();
        self.anchor = None;
        self.suggestions.clear();
        self.entries.clear();
        self.translation = None;
        self.notice.clear();
    }
    fn advance(&self, preserve_huds: bool) -> UiEvent {
        UiEvent::Clear {
            generation: self.clock.fetch_add(1, Ordering::SeqCst) + 1,
            preserve_huds,
        }
    }
    pub fn discard_capture(&mut self) {
        self.buffer.reset();
        self.decoder.reset();
        self.anchor = None;
    }
    pub fn invalidate(&mut self, preserve_huds: bool) -> UiEvent {
        self.discard_capture();
        self.current = None;
        self.selection_position = None;
        self.clear_results();
        self.local_revision = u64::MAX;
        self.model_revision = u64::MAX;
        self.advance(preserve_huds)
    }
    pub fn type_text(&mut self, text: &str) -> UiEvent {
        self.clear_results();
        self.buffer.push(text);
        self.current = Some(self.buffer.snapshot());
        // Advance even if the bounded UI queue cannot accept the Clear event.
        self.advance(true)
    }
}

#[derive(Default)]
pub(crate) struct DisplayGeneration {
    displayed: u64,
    pending_clear: Option<u64>,
}
impl DisplayGeneration {
    pub fn shown(&mut self, generation: u64) {
        self.displayed = generation;
    }
    pub fn accept(&mut self, generation: u64, latest: u64) -> bool {
        if generation != latest {
            return false;
        }
        self.shown(generation);
        true
    }
    pub fn end_frame(&mut self, latest: u64) -> bool {
        if self.displayed == latest {
            self.pending_clear = None;
            return false;
        }
        // Give an in-flight Clear one frame before failing closed on a dropped event.
        if self.pending_clear == Some(latest) {
            self.displayed = latest;
            self.pending_clear = None;
            true
        } else {
            self.pending_clear = Some(latest);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use writy_core::types::SuggestionKind;

    fn with_results() -> Generation {
        let mut generation = Generation::new(Arc::new(AtomicU64::new(7)));
        generation.type_text("hello");
        let snapshot = generation.current.clone().unwrap();
        generation.anchor = Some((
            snapshot.revision,
            FieldSnapshot {
                identity: "fixture".into(),
                text: snapshot.text.clone(),
                caret: 5,
                rect: None,
            },
        ));
        generation.suggestions.push(Suggestion {
            snapshot,
            start: 0,
            end: 5,
            replacement: "Hello".into(),
            description: "fixture".into(),
            kind: SuggestionKind::Spelling,
            provider: "fixture".into(),
        });
        generation.entries.push(LexicalEntry::default());
        generation.translation = Some(("مرحبا".into(), "fixture".into()));
        generation.notice = "Generating…".into();
        generation.selection_position = Some((10, 20));
        generation.local_revision = 1;
        generation.model_revision = 1;
        generation
    }

    #[tokio::test]
    async fn invalidation_cancels_work_and_discards_edit_context_even_when_huds_remain() {
        let mut generation = with_results();
        let task = tokio::spawn(std::future::pending());
        let cancelled = task.abort_handle();
        generation.model = Some(task);
        let previous = generation.id();
        let clear = generation.invalidate(true);
        assert!(
            matches!(clear, UiEvent::Clear { generation: id, preserve_huds: true } if id == previous + 1)
        );
        assert!(generation.current.is_none());
        assert!(generation.buffer.snapshot().text.is_empty());
        assert!(generation.anchor.is_none());
        assert!(generation.selection_position.is_none());
        assert!(generation.suggestions.is_empty());
        assert!(generation.entries.is_empty());
        assert!(generation.translation.is_none());
        assert!(generation.notice.is_empty());
        assert_eq!(generation.local_revision, u64::MAX);
        assert_eq!(generation.model_revision, u64::MAX);
        assert!(generation.model.is_none());
        tokio::task::yield_now().await;
        assert!(cancelled.is_finished());

        let mut display = DisplayGeneration::default();
        assert!(display.accept(generation.id(), generation.id()));
        assert!(!display.end_frame(generation.id()));
        assert!(!display.end_frame(generation.id()));
        assert!(matches!(
            generation.invalidate(false),
            UiEvent::Clear {
                preserve_huds: false,
                ..
            }
        ));
    }

    #[test]
    fn typing_replaces_results_and_lost_clear_fails_closed_after_one_frame() {
        let mut generation = with_results();
        let old_id = generation.id();
        let mut display = DisplayGeneration::default();
        assert!(display.accept(old_id, old_id));
        let (sender, _receiver) = std::sync::mpsc::sync_channel(1);
        sender.try_send(UiEvent::Notice("full".into())).unwrap();
        assert!(sender.try_send(generation.type_text(" world")).is_err());
        let latest = generation.id();
        assert_eq!(latest, old_id + 1);
        assert_eq!(generation.current.as_ref().unwrap().text, "hello world");
        assert!(generation.anchor.is_none());
        assert!(generation.suggestions.is_empty());
        assert!(generation.entries.is_empty());
        assert!(generation.translation.is_none());
        assert!(!display.accept(old_id, latest));
        assert!(!display.end_frame(latest));
        assert!(display.end_frame(latest));
        assert!(!display.end_frame(latest));
    }

    #[test]
    fn delayed_clear_or_current_result_prevents_fallback_and_stale_events_do_not() {
        for delivered in [false, true] {
            let mut display = DisplayGeneration::default();
            display.shown(7);
            assert!(!display.end_frame(8));
            assert!(!display.accept(7, 8));
            if delivered {
                assert!(display.accept(8, 8));
                assert!(!display.end_frame(8));
            } else {
                assert!(display.end_frame(8));
            }
        }
        let mut display = DisplayGeneration::default();
        assert!(!display.end_frame(8));
        assert!(!display.end_frame(9));
        assert!(display.end_frame(9));
    }

    #[test]
    fn explicit_search_capture_reset_preserves_request_and_generation() {
        let mut generation = with_results();
        let id = generation.id();
        generation.discard_capture();
        assert_eq!(generation.id(), id);
        assert_eq!(generation.current.as_ref().unwrap().text, "hello");
        assert_eq!(generation.translation.as_ref().unwrap().0, "مرحبا");
        assert!(generation.anchor.is_none());
        assert!(generation.buffer.snapshot().text.is_empty());
    }
}

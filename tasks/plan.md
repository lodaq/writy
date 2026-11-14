# Implementation plan: assistance controls and terminology

Status: implemented; validation evidence and remaining limitations are in `docs/verification.md`. Tasks are tracked in `docs/checklist.md`, the repository's existing
implementation backlog; existing unfinished work remains intact.

## Outcome

Users independently control assistance on selection, copy, and typing. Temporary pause preserves saved preferences and leaves manual
tools available. Settings and visible labels use the approved terminology in
`docs/terminology-review.md` and `GLOSSARY.md`. Existing suggestions gain Dismiss.

## Architecture decisions

## Ordered task index

1. AC1: independent automatic controls.
2. AC2: automatic copy trigger with deduplication and loop prevention.
3. Checkpoint A: independent automatic flows and manual access.
4. AC3: temporary pause and resume across all automatic flows.
5. AC4: reorganize settings and apply approved visible terminology.
6. Checkpoint B: settings persistence and labels.
7. AC5: dismiss individual suggestions for the current context.
8. AC6: deferred-feature state and documentation reconciliation.
9. Checkpoint C: focused automated checks and desktop acceptance.

## Scope boundaries

The individual-issue Suggestion menu and Arabic ↔ English audio translation
remain deferred. Do not add audio infrastructure.

## Risks and verification

| Risk | Mitigation |
| --- | --- |
| Clipboard events unavailable while Writy lacks focus | Prove on the actual session early; report missing capability before adding infrastructure. |
| Global capture gate couples independent modes | Test all switch combinations and pause separately from feature preferences. |
| Duplicate or self-triggered clipboard requests | Test selection then copy, Writy Copy, repeated external copy, and focus changes. |
| Stale edits after new events or dismissal | Keep generation and target verification checks; test invalidation during pending requests. |
| Screen suggestions immediately return after Dismiss | Compare against unchanged reviewed content, not request IDs alone. |
| Existing uncommitted work overlaps implementation | Inspect the working diff at implementation start and preserve unrelated changes. |

Use focused tests per task, then formatting, workspace tests, and a build at the
final checkpoint. Native desktop validation remains necessary; unit tests cannot
prove clipboard delivery, focus retention, or compatibility with external apps.

# 1. Standalone HUD Architecture and Zero-Animation Policy

Date: 2026-10-24

## Status
Partially superseded by [ADR 0005](0005-bounded-passive-huds.md); retained as design history.

## Context
Writy originally conceived its UI as a single dual-popup window displaying both typing checks on the left and dictionary definitions on the right. As the scope expanded to include rich linguistic categories (collocations, word families, synonyms/antonyms, usage examples, translations), grouping everything into a monolithic window created visual bloat and prevented users from configuring minimal setups (e.g. users wanting only translations and collocations without large definitions). Furthermore, animations on high-frequency typing events introduce perceived friction and lag.

## Decision
1. **Standalone HUD Architecture**: We split the UI into independent, autonomous Wayland layer-shell surfaces ("HUDs") corresponding to distinct semantic categories (`Translation HUD`, `Definition HUD`, `Collocations HUD`, `Word Family HUD`, `Synonyms HUD`, `Examples HUD`, `Suggestions HUD`). Each HUD can be enabled, anchored, and styled independently.
2. **Zero-Animation Policy for High-Frequency Actions**: Keyboard-initiated surfaces (Insert Mode suggestions and the `Alt+G` Translator Modal) must open and close with **0ms animation**. Pre-scripted animations on actions executed hundreds of times per day create perceived latency.
3. **Subtle Springs for Mouse Selection**: Lower-frequency mouse selections use an origin-aware spring transition (140ms ease-out, entering from `scale(0.96)` and `opacity: 0`, never from `scale(0)`).
4. **Origin-Aware Spatial Stacking**: When multiple standalone HUDs trigger simultaneously on word selection, they assemble into a unified vertical stack anchored to the trigger caret/selection coordinates, automatically flipping above if near the screen's bottom boundary.

## Consequences
- The window manager layer manages multiple lightweight layer-shell surfaces instead of one combined window.
- The interface responds with zero perceptual lag during fast typing.
- Users have full agency to toggle individual HUDs in `config.toml`.

# 2. Spatial Ergonomics, Dismissal Physics, and Bilingual Layout

Date: 2026-10-24

## Status
Partially superseded by [ADR 0005](0005-bounded-passive-huds.md); retained as design history.

## Context
Displaying multiple autonomous HUDs (Translation, Definition, Collocations, Family, Syn/Ant, Examples) simultaneously presents three UX failure modes:
1. Vertical overflow: Unconstrained stacking on 1080p laptop displays clips off-screen or obscures the active application.
2. Accidental dismissal: Moving the mouse from a text selection toward a HUD to click controls (such as the audio pronunciation button `▶`) frequently crosses unselected text, triggering premature dismissal in naive implementations.
3. Typographic discordance: Arabic script glyphs at identical nominal point sizes appear significantly smaller than Latin glyphs due to differing x-height baselines.

## Decision
1. **Screen Budget & Height Ceiling**: The combined vertical HUD stack is capped at **380px** (or 40% of display height). Primary HUDs (Translation, Collocations) stay expanded; secondary HUDs (Definitions, Examples) truncate to 2 lines with a subtle bottom blur/fade mask and expand on mouse wheel/hover.
2. **Trajectory-Aware Hover Corridor**: We implement a 14px spatial corridor between the selection origin and the target HUD. The HUD remains stable during cursor transit and dismisses only on an explicit click outside the envelope or pressing `Esc`.
3. **Bilingual Typography & Optical Scaling**: Arabic text is dynamically scaled up by **+15%** relative to Latin typography to achieve optical baseline parity. The layout automatically mirrors (RTL ↔ LTR) depending on the source language.

## Consequences
- HUDs never clip off-screen regardless of how many categories are active.
- Clicking interactive buttons inside HUDs is fluid and reliable with zero accidental dismissals.
- Arabic text matches English text in visual presence and legibility.

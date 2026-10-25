# 001 — Make HUD feedback legible without moving live text

- **Status**: DONE
- **Commit**: e50359d
- **Severity**: MEDIUM
- **Category**: Feedback, accessibility
- **Estimated scope**: native HUD component, shared copy control, GTK stylesheet, visual tests

## Recon and audit

Writy uses GTK4, GTK CSS and layer-shell, with no animation library or motion tokens.
Live typing and keyboard search are high-frequency. The product is a quiet writing
companion. The existing absence of live entrance/reflow animations is deliberate.

| Severity | Location | Before | After | Why |
| --- | --- | --- | --- | --- |
| HIGH | crates/writy-linux/src/ui.rs, View::show | Per-card fixed height and body ellipsis | One outer scroller, full wrapping bodies | The user cannot read clipped data |
| MEDIUM | crates/writy-linux/src/ui/components.rs, hud_copy_button | Immediate icon replacement | 125ms native opacity crossfade | Legible confirmation without width changes |
| LOW | crates/writy-linux/src/ui.css, HUD buttons | Colour-only press | Scale .97 over 160ms strong ease-out | Quiet pointer feedback |

## Opportunities

| Location | Today | Purpose | Frequency | Suggested motion |
| --- | --- | --- | --- | --- |
| HUD Copy | Icon snaps | State indication | Occasional | GTK Stack crossfade, opacity, 125ms; native GTK curve; no movement |
| HUD toolbar buttons | Static press | Feedback | Tens/day | GTK CSS transform scale(.97), 160ms cubic-bezier(0.23,1,0.32,1) |

Rejected: live HUD entrances and body updates (keyboard/high-frequency); scroll
position/layout animation (functional reading); category stagger (delays reading);
bouncy expansion (no gesture momentum).

## Target

Use a single native scrollable reading stack with seven logical sections, stable
shared chrome, wrapped top-aligned bodies and full attribution. Outer geometry
stays inside the monitor. Only pointer controls animate. GtkSettings
gtk-enable-animations=false removes transforms/transitions and uses static copy
confirmation. No browser media queries in GTK CSS.

## Repo conventions

Use gtk widgets and semantic GTK theme colours, with shared components under
crates/writy-linux/src/ui/. Preserve the namespace "writy", KeyboardMode::None,
screenshot layer handling and the backend's independent edit invalidation.

## Steps

1. Extract HUD construction, content update and bounds into ui/hud.rs.
2. Replace seven top-level surfaces with one outer scroller; all enabled sections
   remain present and no body uses ellipsis or a fixed height.
3. Keep Copy per section; move Open, expand-reading-area and Dismiss to shared chrome.
4. Implement the two feedback recipes above, preserving instant keyboard/live updates.
5. Update native regression tests to verify wrapping, scroll reachability, bottom
   content, Arabic text, narrow displays, text scaling and reduced motion.

## Boundaries

Do not change model generation, dictionary evidence, screenshot/capture safety or
the user's keyboard bindings. Add no dependencies.

## Verification

Run cargo test --workspace, cargo fmt --check, strict cargo clippy, the opt-in
native component gallery and tests/desktop/run.py. Render top and bottom of a
long bilingual stack. Confirm no cropped glyphs, all sections reachable, no panel
overlap, and no scroll reset on a same-query model update. Toggle GTK animations
off and confirm static feedback. Slow press feedback to 5x for visual inspection,
then restore 160ms. DONE when those checks pass and the installed app is restarted.

Verified: workspace tests, strict Clippy, native Wayland component gallery, and two consecutive desktop integration runs. The release was rebuilt and the user service restarted.

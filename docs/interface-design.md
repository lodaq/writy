# Interface design

Writy is a native GTK writing companion. Its interface adapts the supplied
Apple design principles to native controls, desktop typography and keyboard
navigation. The intended feeling is calm, immediate and predictable.

## Component contract

## Native accessibility and materials

Typography inherits the desktop font size. Colours follow the GTK theme,
including dark and high-contrast themes. Surfaces are opaque: the UI does not
depend on compositor blur or transparency for legibility. No decorative motion,
sounds, haptics or gesture interactions are added. Native control animation
continues to follow GTK's animation setting, while result state changes are
instantaneous and do not move content across the screen. Pointer press feedback
uses scale .97 over 160ms with cubic-bezier(0.23, 1, 0.32, 1); Copy confirms with
a 125ms native icon crossfade. gtk-enable-animations=false removes the transform
and crossfade while retaining static colour/icon feedback.

All surfaces share the approved search reference's neutral materials, blue
accent, rounded controls and source hierarchy. Search keeps its larger reading
scale; preferences and floating panels use a denser desktop scale. Native window
controls, focus navigation, tabs, dropdowns and switches remain native GTK.
This adapts Apple HIG [Settings › Best practices](https://developer.apple.com/design/human-interface-guidelines/settings),
[Tab views › Best practices](https://developer.apple.com/design/human-interface-guidelines/tab-views),
and [Buttons › Style](https://developer.apple.com/design/human-interface-guidelines/buttons)
to a Linux desktop: peer preferences stay discoverable, secondary actions stay
quiet, and every page supports larger text without losing its controls.

## Verification

Run the ordinary checks:

```sh
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

An opt-in synthetic component gallery lives in
`crates/writy-linux/src/ui_tests.rs`. It starts no backend and uses no captured
prose. Use a disposable configuration pointing Ollama at an unavailable local
port to avoid contacting your configured model endpoint. The gallery writes
PNG files to `WRITY_UI_SNAPSHOTS` (default `/tmp/writy-ui`).

```sh
XDG_CONFIG_HOME=/path/to/disposable-config \
GSK_RENDERER=cairo \
cargo test -p writy-linux native_component_gallery -- --ignored --test-threads=1
```

This requires a graphical session and a window manager that honours fixed-size
test windows. It covers light/dark results, a narrow window, settings at both
ends of its scroll range, all seven HUD sections, long bilingual text, large
desktop text, scroll retention, expansion, reduced motion, loading and empty results.
For reliable Wayland rendering in an isolated compositor, run
`python tests/desktop/gallery.py`. This starts no backend or bridge and captures
only the synthetic test windows.
Renderer snapshots complement, rather than replace, keyboard/screen-reader
testing and the nested compositor integration test described in
`tests/desktop/README.md`.

Search results belong to the explicit search input. Pointer clicks, selection,
caret movement and capture/keymap resets must not clear that query, its results,
or an in-flight request. Actual input edits and explicit lookup/translation
commands can replace results; dismissal and security clearing still apply.

Search categories are native keyboard-accessible tabs. Each populated category
has its own scrollable page and copy action; the main Copy result action follows
the selected tab. Source attribution and media live on a separate Sources tab.
The selected category persists when asynchronous results update. Empty categories
are omitted, and a nonempty input never uses the initial welcome prompt.
Explicit search sessions survive ambient focus, capture and bridge resets;
reopening the modal reruns a retained query if there is no new input to seed it.
Selection lookups sample the Hyprland pointer once, then place the reading HUD
12 pixels beside it. Placement flips above/left and clamps to the output edges;
model updates use the captured anchor rather than following pointer movement.

## Source groups and live updates

Each result tab groups rows by dictionary or model, with the source heading
immediately above its content and a visible divider between sources. Headwords
identify distinct entries in search, with part of speech beside them when supplied.
Repeated senses under the same headword share a heading. Exact duplicate records
within the same source, headword and part of speech are shown once; independent
sources and distinct headwords remain separate. Copy contains the source text,
without presentation numbers. Publisher markup is retained only where it maps to
one definition exactly. Floating panels keep the query in their header.

Search follows the supplied 1402 × 1122 reference: proportional side insets
(20–80 logical pixels), an inline brand/language/settings header, automatic lookup and improvement, and one bordered reading surface containing its tabs.
The charcoal surface and blue primary action retain theme-aware text and a
light appearance; typography scales with the desktop font. Narrow windows stack
the header and actions, wrap text, and
scroll the results; larger system fonts can also scroll the whole form rather than
clip controls or the footer. Arabic headwords lead each RTL entry, with sense
numbers at the right and multiline details together underneath. Arramooz's
empty formatting lines are removed for display and adjacent verb attributes
share a compact line; copying retains the original dictionary text. Typing automatically translates between English and Arabic when dictionaries have no translation. Suggestions stays available for both words and phrases, with individually applicable corrections and alternatives. Search Apply replaces the input only if it still matches the suggestion source; floating Apply uses the verified external-edit path. The former top lookup and Copy result buttons are hidden; per-section and per-choice Copy actions remain. Translation remains available in either language, and the selected tab survives loading and language changes. Alt+G toggles the search window without pausing background assistance. Compact line spacing is shared across labels. Pango handles
logical text alignment: adding right justification to an RTL label would mirror
its paragraph a second time and misplace it.

This follows Apple HIG [Layout › Visual hierarchy](https://developer.apple.com/design/human-interface-guidelines/layout),
[Typography › Conveying hierarchy](https://developer.apple.com/design/human-interface-guidelines/typography),
and [Right to left › Text alignment](https://developer.apple.com/design/human-interface-guidelines/right-to-left),
using GTK controls, installed fonts and semantic theme colors. Native GTK/Xvfb
captures cover Arabic and English, dark and light themes, narrow layouts, larger
text, scrolling and the existing copy/tab behavior.

Typing, deletion and navigation invalidate correction anchors without hiding
the reference window. Typed text refreshes the visible content immediately;
unchanged groups preserve selection and scroll position. Explicit dismissal,
secure fields and capture failures retain their existing hiding behavior.

Selecting a word, sentence or passage opens the Translation tab immediately,
including when no dictionary matches. A loading message stays visible until
the full selected text is translated; model failures remain visible in that tab.

## Completed plan: make HUD feedback legible without moving live text

Historical implementation record; current layout guidance appears above.

- **Status**: DONE
- **Commit**: e50359d
- **Severity**: MEDIUM
- **Category**: Feedback, accessibility
- **Estimated scope**: native HUD component, shared copy control, GTK stylesheet, visual tests

### Recon and audit

Writy uses GTK4, GTK CSS and layer-shell, with no animation library or motion tokens.
Live typing and keyboard search are high-frequency. The product is a quiet writing
companion. The existing absence of live entrance/reflow animations is deliberate.

| Severity | Location | Before | After | Why |
| --- | --- | --- | --- | --- |
| HIGH | crates/writy-linux/src/ui.rs, View::show | Per-card fixed height and body ellipsis | One outer scroller, full wrapping bodies | The user cannot read clipped data |
| MEDIUM | crates/writy-linux/src/ui/components.rs, hud_copy_button | Immediate icon replacement | 125ms native opacity crossfade | Legible confirmation without width changes |
| LOW | crates/writy-linux/src/ui.css, HUD buttons | Colour-only press | Scale .97 over 160ms strong ease-out | Quiet pointer feedback |

### Opportunities

| Location | Today | Purpose | Frequency | Suggested motion |
| --- | --- | --- | --- | --- |
| HUD Copy | Icon snaps | State indication | Occasional | GTK Stack crossfade, opacity, 125ms; native GTK curve; no movement |
| HUD toolbar buttons | Static press | Feedback | Tens/day | GTK CSS transform scale(.97), 160ms cubic-bezier(0.23,1,0.32,1) |

Rejected: live HUD entrances and body updates (keyboard/high-frequency); scroll
position/layout animation (functional reading); category stagger (delays reading);
bouncy expansion (no gesture momentum).

### Target

Use a single native scrollable reading stack with seven logical sections, stable
shared chrome, wrapped top-aligned bodies and full attribution. Outer geometry
stays inside the monitor. Only pointer controls animate. GtkSettings
gtk-enable-animations=false removes transforms/transitions and uses static copy
confirmation. No browser media queries in GTK CSS.

### Repo conventions

Use gtk widgets and semantic GTK theme colours, with shared components under
crates/writy-linux/src/ui/. Preserve the namespace "writy", KeyboardMode::None,
screenshot layer handling and the backend's independent edit invalidation.

### Steps

1. Extract HUD construction, content update and bounds into ui/hud.rs.
2. Replace seven top-level surfaces with one outer scroller; all enabled sections
   remain present and no body uses ellipsis or a fixed height.
3. Keep Copy per section; move Open, expand-reading-area and Dismiss to shared chrome.
4. Implement the two feedback recipes above, preserving instant keyboard/live updates.
5. Update native regression tests to verify wrapping, scroll reachability, bottom
   content, Arabic text, narrow displays, text scaling and reduced motion.

### Boundaries

Do not change model generation, dictionary evidence, screenshot/capture safety or
the user's keyboard bindings. Add no dependencies.

### Verification

Run cargo test --workspace, cargo fmt --check, strict cargo clippy, the opt-in
native component gallery and tests/desktop/run.py. Render top and bottom of a
long bilingual stack. Confirm no cropped glyphs, all sections reachable, no panel
overlap, and no scroll reset on a same-query model update. Toggle GTK animations
off and confirm static feedback. Slow press feedback to 5x for visual inspection,
then restore 160ms. DONE when those checks pass and the installed app is restarted.

Verified: workspace tests, strict Clippy, native Wayland component gallery, and two consecutive desktop integration runs. The release was rebuilt and the user service restarted.

Panel titles and section tabs retain their full names. Never ellipsize, abbreviate,
or reduce their text size to fit. Scrollable tabs provide access when space is limited.

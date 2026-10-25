# Interface design

Writy is a native GTK writing companion. Its interface adapts the supplied
Apple design principles to native controls, desktop typography and keyboard
navigation. The intended feeling is calm, immediate and predictable.

## Component contract

- Search: one primary lookup action, a separate translation action, labelled
  multiline input, and a scrollable result collection. Copy is unavailable until
  a result exists. Welcome, generating and no-result states explain the next step.
- Results: category and copy action share a header; readable selectable content
  follows, then attribution. Copy confirmation stays on the control that caused it.
  Status messages are never offered as copyable writing suggestions.
- Floating panels: seven logical categories share one bounded, scrollable
  reading surface. Bodies wrap and take their natural height; no body is ellipsized
  or constrained to a strip. There is one visible scrollbar, persistent shared
  Open/Expand/Dismiss controls, and a Copy action per section. Attribution remains
  below each body. Updates to the same query retain the reading position.
  The compact area uses up to 72% of monitor height (maximum 680 logical pixels);
  Expand uses the monitor minus 48 pixels. Short results size to their content.
  The surface never takes keyboard focus; Alt+G provides a keyboard-accessible
  search view. Non-editing screenshot keys and temporary focus/shortcut resets
  retain displayed reference text while the backend invalidates correction
  anchors. Ordinary app switches, secure fields, locks, new typing and explicit
  dismissal still clear stale results. During Flameshot capture, the HUD moves
  below the fullscreen annotation surface; screenshot annotations and selections
  are excluded from automatic writing capture.
- Settings: privacy, models, floating panels and dictionaries form named groups.
  Installed model choices and cloud configuration are disclosed on demand.
  Save and operation feedback remain outside the scrolling form. Reopening settings
  presents the existing window. Escape dismisses the window.
- Controls: immediate native press feedback, a visible keyboard focus ring,
  semantic theme colours and explicit disabled states. Text and labels wrap
  within narrow containers; input direction follows English or Arabic.

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

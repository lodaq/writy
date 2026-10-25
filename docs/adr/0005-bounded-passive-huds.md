# 5. Bounded passive HUDs and native text

Date: 2026-10-25

## Status

Accepted. Retains ADR 0001's independent category surfaces and animation-free
keyboard updates. Supersedes its mouse spring and perceptual-latency guarantee,
and ADR 0002's hover corridor, blur expansion and fixed Arabic scaling choices.
Those original documents remain as design history.

## Decision

Use seven independently enabled GTK layer-shell surfaces with one placement
manager. Keep passive surfaces unfocused and updates animation-free. Constrain
the entire stack to `min(380 logical pixels, 40% of output height)`, choose the
active output, use valid caret geometry when available, and otherwise fall back
to the output corner. Reject geometry outside that output.

Preserve legible category slots. When every available category cannot fit, show
an overflow indicator and expose the complete results in focused search rather
than squeezing each HUD into unreadable text. Use native scrolling in search.
Dictionary markup passes a small Pango allowlist; arbitrary HTML is not rendered.
Use GTK bidirectional layout and script-capable fonts, with opaque backgrounds.
A fixed Arabic percentage is not an established legibility requirement.

Dismiss on a replacement query, source focus/context change, explicit command,
or a reported cleared selection. Crossing ordinary text is not a dismissal
signal. A click on Writy's own controls must not invalidate its result before
Copy can run. No universal visual-deselection detector or hover corridor is
claimed.

## Evidence and consequences

The isolated GTK/Hyprland fixture verifies painted, nonoverlapping HUDs inside
the viewport budget and a focused mixed-script search window. Actual mouse
interaction, output scaling and font preferences remain compatibility-trial
work. Neither these choices nor the earlier prototypes guarantee zero latency,
perfect typography or a fixed memory footprint.

## Amendment — all insert-mode categories, 2026-10-25

The user requested every enabled HUD while typing and opted into labeled AI
lexical content. Insert mode therefore expands beyond the original 380/40%
reading budget to fit all enabled categories within the output. Pending and
unavailable categories remain visible. Reading retains overflow into search.
Current-word dictionary lookup runs before generated enrichment; dictionary
evidence is never replaced by model claims.

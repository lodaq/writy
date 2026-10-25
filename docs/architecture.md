# Architecture

## Components

`writy-core` owns domain types, configuration, a versioned typing buffer, SQLite
lexical storage/imports, English linting, and model-provider requests. It must not
import GTK or the Hyprland plugin API. Dictionary entries carry attribution;
model output is marked as generated rather than attributed to a dictionary.

`writy-linux` builds the `writy` executable. It owns CLI/control sockets, the
GTK4/layer-shell surfaces, primary selection, keyboard decoding, focus policy,
accessibility inspection, and replacement decisions. It invokes the engine and
checks that results still belong to the current focus/text generation.

`bridge/hyprland` is a small C++ compositor plugin. It observes input/focus and
forwards bounded protocol records to the daemon. It does not perform network
requests, model generation, dictionary work, or blocking socket I/O on the
compositor event loop. It never grabs or consumes the user's ordinary keys.
See [its protocol/build guide](../bridge/hyprland/README.md).

The daemon listens on runtime Unix sockets inside `$XDG_RUNTIME_DIR/writy/`.
The bridge uses `bridge.sock` with newline-delimited JSON, protocol version 1,
sequence numbers, and focus epochs. It starts with capture disabled; the daemon
must authorize the current focus epoch. A gap, disconnect, or invalid context
resets the projected typing state.

## Request and edit flow

Selection or search produces a reading request. Newly typed English prose
produces a writing snapshot after the configured idle interval. Automatic model
requests additionally require the text to end with a dot (`.`), ignoring trailing
whitespace; explicit searches and translation commands remain immediate. The snapshot
contains text, source, focus epoch, revision, language, and confidence. Dictionary
and linter work can render before a model response arrives. Model requests have
bounded timeouts; only the current generation may update HUDs.

A key stream provides projected text, not proof of the application's current
contents. Accessibility information can verify context in participating apps.
Accept checks the current target and source text immediately before an edit;
when verification fails or is unavailable, Copy remains available. Never inject
backspaces based solely on an old projected buffer.

The AT-SPI broker confines native calls to a dedicated worker, bounds its queue
and calls, and identifies fields by process, bus name, and object path. Before a
single `SetTextContents`, it rechecks the focused field, full text, caret, and
typed segment. AT-SPI offers no atomic compare-and-set; an application edit
between the final read and write remains a limitation to test in the trial.

## HUD and focus ownership

The seven independently configurable categories share one layer-shell reading
surface. Each section wraps naturally; one outer scroller makes all content
reachable without fixed-height strips. The default viewport uses up to 72% of
output height, capped at 680 logical pixels. Expanded reading uses the output
height minus 48 pixels; placement remains clamped to that output. The shared
toolbar and footer stay outside the scroll area. Use native GTK text/image widgets
and sanitized dictionary formatting rather than a web engine. Treat local media
paths as untrusted dictionary input.

Ambient HUDs leave focus in the source app. Search is a focused window and should
not feed its own typing into ambient assistance. Typing results open/close
immediately; selection motion must respect reduced-motion preferences. Values
from the original design notes are prototypes until validated on real text.

## Data, providers, and privacy

SQLite uses WAL for concurrent reading during imports and indexes alias entry
IDs for efficient source replacement. Lock contention fails within 250 ms
rather than blocking the desktop service for multiple seconds.
It stores imported lexical entries and attribution; it does not provide a
persistent writing history. Import should be transactional and repeatable.
Dictionary packages and media are separate from the application license.

Ollama runs on a loopback endpoint. The optional cloud provider uses an HTTPS
OpenAI-compatible endpoint and an API key obtained from a named environment
variable. With cloud configured, eligible text can be sent automatically. Offline
mode blocks cloud requests; service failures must not stop local dictionary and
proofreading work. Do not log typed text, clipboard contents, or credentials.

Password roles and configured app exclusions are best effort, not proof of
privacy in opaque fields. Terminal prose explicitly permits capture for the
current focus; it cannot recognize every shell/password state. Operating-system
swap/core behavior and provider retention remain outside an in-memory-buffer
claim. The supplied user service disables core dumps for its process.

## Insert-mode enrichment

After local current-word lookup and Harper checking, one structured model request
provides sentence translation and lexical details for the final word. A local
Ollama JSON schema constrains field types and sizes; compatible cloud responses
are validated before fallback. Rewrite work follows sequentially once the
configured word threshold is met. Superseded work is cancelled and every response
still requires its original snapshot. Generated categories fill only gaps in
dictionary evidence and carry visible AI labels; they are not persisted. Empty
or inapplicable categories keep an explicit visible state in insert mode.

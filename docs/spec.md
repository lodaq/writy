# Product specification

Status: accepted direction for the initial personal app; implementation and
runtime validation are tracked separately in [tasks](../tasks/README.md).

## Goal and audience

Help an Arabic-speaking English reader and writer on Arch/Hyprland understand
text and express sentences naturally without repeatedly switching applications.
Reading and writing are both first-release capabilities; sentence phrasing is
the primary writing value. This is a personal tool before it is a general product.

## Required behavior

- **Reading:** English↔Arabic word lookup and sentence translation from primary
  selection or focused manual search. Preserve source attribution and available
  definitions, examples, collocations, related words, pronunciation, and media.
  Missing reading categories stay absent. In insert mode, enabled categories stay
  visible; missing lexical details may be filled by explicitly labeled AI content.
  Dictionary evidence takes precedence over generated content.
- **Writing:** English spelling/grammar through Harper, basic word completion,
  and model phrasing from a newly typed sentence buffer. Query after 300 ms idle
  and two words by default for rewriting. Insert-mode word details and sentence
  translation can start after a word has at least two characters. Arabic
  reading/translation is included; Arabic
  rule-based grammar correction is deferred.
- **Capture:** The Linux adapter observes supported GUI typing through a native
  Hyprland bridge without taking an exclusive keyboard grab or consuming normal
  keys. Reset on focus, pointer, layout, navigation, modified shortcuts, and any
  event that makes the projected buffer unreliable. Do not pretend to recover
  arbitrary existing document text from a key stream.
- **Context safety:** Block recognized password roles and configured apps.
  Detection is best effort in opaque input. Terminal prose is opt-in for the
  current focus only and resets on focus change. The user accepts that hidden
  password prompts may remain indistinguishable from ordinary terminal input.
- **Actions:** `Alt+G` opens search; `Ctrl+Alt+Enter` accepts a suggestion. Apply
  replacements only against a verified current text range/context. A projected
  buffer, changed focus, or stale generation permits Copy instead of editing.
- **Providers:** Use local dictionaries and a local-model path offline. When a
  cloud provider is configured and offline mode is off, requests are automatic
  in enabled apps. Keep at most one active request per backend, cancel superseded
  work where possible, and discard stale results even if cancellation fails.
- **Surfaces:** Seven independently configurable HUD sections: translation,
  definition, examples, collocations, synonyms/antonyms, word family, suggestions.
  Use one floating reading stack with naturally wrapped bodies and one outer
  scrollbar. The compact viewport uses up to 72% of output height, capped at
  680 logical pixels; Expand uses the output height minus 48 pixels. All enabled
  sections remain reachable, including pending/unavailable categories. Clamp
  placement to the active output. Manual search takes focus; ambient HUDs do not.


## Data and platform boundary

The first dictionary path uses open Wiktionary/FreeDict data packs and imported
StarDict data with local media. Keep dataset licenses and source metadata with
imports. Start with SQLite; add another index only after a measured limitation.
Keep OS-independent domain and provider code apart from Linux presentation and
capture. Windows/macOS, MDict, ZIM/Wikipedia, Arabic rule grammar, and a commercial
dictionary catalog are later work.

No universal application-support, constant-memory, sub-millisecond lookup, or
translation-quality guarantee is made. An app is supported only after the
compatibility scenarios below pass in that app/version.

## Acceptance

1. Run focused lookup, selection lookup, new typing, reset, and Copy in the user's
   browser, GUI editor, messaging input, and terminal prose workflow. Record exact
   app/compositor versions and which AT-SPI capabilities were available.
2. Switch windows, edit text, click elsewhere, and change layout while requests
   run. Old responses must never replace current results or application text.
3. Reject unverified text replacement. Normal typing continues if the daemon or
   bridge disconnects, restarts, or overloads. Observe no lost keystrokes.
4. Exercise offline mode, absent models, cloud timeouts, malformed output,
   missing dictionary/media, mixed direction text, and long results. Show a
   useful status and retain local results where available.
5. Evaluate the 50 committed cases, exactly 25 reading and 25 writing. A human
   marks useful/not useful after observing the result; the goal is at least 40
   useful cases. Complete a one-week personal trial before calling v1 validated.

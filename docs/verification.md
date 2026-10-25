# Verification record

Initial implementation, 2026-10-25. This is an engineering record, not a release
approval or a claim of universal compatibility.

## Environment

- Arch Linux, Hyprland 0.56.2 at efb50993780079460b0cbed1363e2166a2de1d9f.
- Intel i7-9850H, 32 GB RAM, NVIDIA Quadro T2000 with 4 GB VRAM.
- GTK 4.24.1, gtk4-layer-shell 1.3.0, AT-SPI 2.62.1.

## Completed evidence

- Rust debug and release builds pass. Formatting and Clippy with warnings denied
  pass; 50 Rust tests pass (32 core, 6 HTTP integration, 12 Linux). Both native
  CTest tests pass.
- Native bridge compiles against installed compositor headers; CTest covers
  reconnect, bounded queues, disconnects and the dedicated acceptance chord.
- Actual nested Hyprland proof: real virtual-keyboard events, projected prose,
  Harper findings, actual Ctrl+Alt+Enter verified GTK TextView correction, automatic
  PRIMARY-selection lookup, recognized PasswordEntry
  exclusion, mixed English/Arabic delivery, daemon SIGSTOP/exit passthrough,
  plugin unload, dictionary lookup/copy, focused search, and a painted lexical
  stack staying within its viewport budget. The parent compositor was never a
  plugin-load target.
- Actual official FreeDict downloads were SHA512 verified and imported in an
  isolated directory: 87,424 EN→AR and 52,996 AR→EN entries. Known-word lookups
  succeeded; those counts do not establish dictionary quality.
- HTTP fixtures exercise offline cloud-stub exclusion before prose is sent,
  authentication/rate-limit errors, interrupted/truncated/oversized responses,
  secret-safe diagnostics and visible fallback failure notices.

## Local model observations

Synthetic short prose only; no paid cloud endpoint was called. These are single
observations, not p50/p95 latency guarantees or a usefulness score.

| Candidate | Observation |
|---|---|
| TranslateGemma 4B | EN→AR cold 12.31 s, warm 1.10 s; AR→EN warm 1.37 s. Partial CPU/GPU placement was observed. |
| Qwen3 4B, installed old template | `think:false` and `/no_think` still produced reasoning and a token-limit stop. Incomplete results are rejected. |
| Gemma3 4B | A cold short rewrite completed in 12.31 s with a meaning-preserving output. Used as the initial rewrite default; model remains configurable. |

Ollama's `/api/show` omitted cloud-routing fields for the installed remote stub;
`/api/tags` included them. Writy checks both and requires verified local GGUF
metadata. A localhost URL alone is not evidence of offline inference.

## Reproducible measurements

Run synthetic offline generation with:

```sh
cargo run -p writy-core --example offline_smoke --locked
```

The public ModelService path completed both translation directions and both
rewrite languages using its local-metadata checks and stream parser. Observed
elapsed times were 15,633 ms EN→AR, 1,067 ms AR→EN, 13,456 ms English rewrite,
and 1,635 ms Arabic rewrite. The first call for each model includes loading;
these four samples are not percentiles. No cloud service was contacted.

With both official FreeDict packs installed (140,420 entries), 800 warmed
SQLite lookups across eight fixed terms measured p50 **37 μs**, p95 **57 μs**;
six of eight sample terms matched. Import/source enumeration had already warmed
the filesystem. This measures neither cold-disk latency nor dictionary quality.
Reproduce with `cargo run -p writy-core --example dictionary_benchmark -- DATABASE`.

The release daemon used **329,740 KiB RSS** after the synthetic nested desktop
sequence, with Harper loaded, a tiny dictionary and GTK windows. Models run in
Ollama separately; this is one observation, not a fixed memory budget.

The final synthetic desktop run used `target/release/writy` and the rebuilt
bridge. It also confirmed translation copies omit UI labels and multiple HUDs
fit the viewport budget without overlapping. Its private fixture data and logs
were retained at `/tmp/we-8911qpi9` for local inspection. The binding harness
sets symbol-based resolution for virtual keyboards' custom keycodes.

Basic StarDict formatting is converted to an allowlisted Pango subset (bold,
italic, underline, strikeout, sub/superscript); arbitrary attributes and active
content are discarded. Local image dimensions and decode size are bounded.

## Remaining release gates

- Actual Brave, Electron applications, terminal prose, physical hotplug,
  Compose/IME behavior, multiple outputs/scaling and user keymaps.
- Human ratings on tests/fixtures/evaluation.json and the one-week trial.
- Clean Arch package installation, hosted CI execution and dependency advisory
  audit (`cargo-audit` is not installed in this development environment).
- Real configured cloud-provider validation; only protocol fixtures were used.

AT-SPI verification checks field identity, process, original text and caret
immediately before a single SetTextContents call. AT-SPI supplies no atomic CAS;
a final application-side read/write race remains. Unknown contexts use Copy.

## Installed desktop follow-up — 2026-10-25

The user authorized main-session installation after the nested proof. The
release executable and ABI-matched plugin are installed under `~/.local`; a
user service and marked Hyprland Lua section manage startup and shortcuts.
Configuration reload reports no errors; all six bindings are registered.
See [local setup and undo](desktop-setup.md).

A repeated FreeDict install was found holding the database write lock for
minutes. Source replacement lacked an alias foreign-key index, and rollback
journaling blocked readers. The interrupted import rolled back. Added that index,
WAL, bounded 250 ms lock waits, a red/green concurrent-import read regression,
and operational startup-error diagnostics. The final count is 47 passing Rust
tests; formatting, Clippy with warnings denied and release build pass.

On the actual installed daemon, both sources were then reimported while status
commands were sampled: EN→AR 3,296 ms (33 status samples, slowest 175 ms); AR→EN
1,861 ms (20 samples, slowest 98 ms). Both counts match the original datasets.
This is a source-replacement observation, not an import-time guarantee.

The main-session synthetic GTK fixture passed live projection, Harper output,
AT-SPI verification, real Ctrl+Alt+Enter through the installed Lua binding, and
recognized password exclusion. Symbol-based resolution was temporarily enabled
for the virtual keyboard; the previous setting was restored. No existing user
field was used as a typing target. The fixture was removed afterward.

This adds one real installed configuration to the evidence; Brave/Electron,
physical keyboards/Compose, terminal prose, multiple outputs and the human
usefulness trial remain release gates.

## All insert-mode HUDs — 2026-10-25

The user requested every enabled HUD while typing and chose labeled AI content
for categories absent from the installed translation dictionaries. Insert mode
now looks up the final word locally, shows all enabled category surfaces with
explicit pending/unavailable states, and expands its height budget within the
output. Reading keeps the original smaller budget and overflow behavior.

A schema-constrained Ollama request supplies sentence translation and lexical
categories; rewriting follows in a separate request. Dictionary evidence wins
per category, generated rows visibly carry `AI`, and new typing clears old
lexical results. Snapshot checks and cancellation cover both requests. The new
structured-output and dictionary-precedence tests bring the total to 49 passing
Rust tests. Clippy, formatting and release build pass.

A real local Gemma3 4B structured request returned all lexical fields in 21,088 ms
for fixed synthetic text. It still produced lexical inaccuracies (including an
invented derivative), so schema validity is not evidence of linguistic quality.
The model's generated content remains labeled and needs human evaluation.
Combining rewriting into that same structured task changed the original meaning
in an earlier sample; the implementation therefore keeps rewriting separate.

The nested desktop screenshot at `/tmp/writy-all-huds.png` shows all seven
surfaces together with generated content. The installed user service has been
updated to this release executable. Output geometry changes trigger relayout;
fixed viewport sizing prevents previous HUD sizes from carrying into new stacks.

The resize regression exposed stale GTK monitor mode dimensions in the nested
compositor. Focus metadata now includes the compositor's logical output bounds,
with geometry-only refreshes that do not reset projected typing. Placement uses
those bounds and retains GTK geometry as a fallback for older bridges.

Final release-binary desktop run with `--with-model` passed all checks, including
seven painted insert-mode surfaces, generated lexical details, verified chord
acceptance, reading height budget, PRIMARY lookup, Copy, search, password
exclusion, mixed scripts, daemon hang/exit passthrough and plugin unload.
Synthetic diagnostics are at `/tmp/we-so3w205x`. Both installed artifacts were
updated and the user service reconnected to the new bridge.

## Ollama model selection — 2026-10-25

Settings now discovers all installed tags asynchronously from the configured
Ollama `/api/tags` endpoint and presents searchable writing/translation
selectors plus Refresh. Existing selections survive discovery failures or a
missing tag; Save uses the selected tag and reloads the daemon configuration.
`writy models` also lists all tags. A real metadata request and the installed
CLI each returned 17 tags. The installed Settings window was visually checked
with both selectors. Catalogue validation brings the suite to 50 passing Rust
tests; Clippy and the release build pass. Metadata discovery sends no prose and
does not change the existing offline inference checks.

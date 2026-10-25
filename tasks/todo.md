# Implementation and validation checklist

Status key: `[ ]` pending; `[x]` completed with linked evidence. A build or mock
HTTP test is not proof that the app works in a live Hyprland session. This file
is the implementation backlog; completed implementation/check items are supported
by [the verification record](../docs/verification.md).
The [spec](../docs/spec.md) owns behavior.

## 1. Foundation and data

- [x] Rust workspace builds with `writy-core` and the Linux `writy` binary.
- [x] Configuration, versioned snapshots, reset behavior, stale-result checks.
- [x] SQLite lexical storage, attribution, transactional import, local media.
- [x] Open bilingual pack installation and StarDict import, with license metadata.
- [x] English Harper linting and basic word completion.
- [x] Ollama and configurable HTTPS OpenAI-compatible providers, offline fallback.

## 2. Linux integration

- [x] Native bridge compiles against the supported Hyprland build; transport tests pass.
- [x] Nested-session proof of focus, keymap, modifiers, sequence gaps, and reconnect.
- [x] Daemon/control CLI, primary-selection watch, and focused search.
- [x] Seven independently configurable category HUDs with one coordinated stack.
- [x] AT-SPI capability inspection, recognized secure-field exclusions, blocklist.
- [x] Per-focus terminal prose and reset policy.
- [x] Verified replacement and Copy fallback; no speculative backspace injection.

## 3. Packaging and automated checks

- [x] `cargo fmt --all -- --check`.
- [x] `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- [x] `cargo test --workspace --locked` and release build.
- [x] CMake build and CTest transport tests.
- [ ] Arch package installation in a clean build environment.
- [ ] GitHub Actions has passed on the repository; adding the workflow is not a run.

The implemented terminal policy and capture safeguards still need the real-app
compatibility checks below. Rust evidence: **50 passing tests**, Clippy with
warnings denied, formatting, and release build. Native evidence: **2 passing
CTest tests** plus the isolated desktop run.

## 4. Personal acceptance trial

- [ ] Record a compatibility matrix for the user's actual app/version combinations.
- [ ] Demonstrate no lost typing with daemon/plugin disconnect and restart.
- [ ] Demonstrate no stale edits after focus, selection, navigation, and layout changes.
- [ ] Confirm unknown/changed fields use Copy when replacement is unverified.
- [ ] Run 25 reading and 25 writing cases in `tests/fixtures/evaluation.json`.
- [ ] Human usefulness result reaches 40/50; leave ratings blank until evaluated.
- [ ] Complete a one-week personal trial; record unresolved friction and regressions.

## Later

MDict, ZIM/Wikipedia, Arabic rule grammar, more dialect coverage, commercial
source catalogs, Windows/macOS adapters, and specialized FST indexing remain
outside the initial implementation. Revisit indexing after measured need.

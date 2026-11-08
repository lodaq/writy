# Implementation and validation checklist

Status key: `[ ]` pending; `[x]` completed with linked evidence. A build or mock
HTTP test is not proof that the app works in a live Hyprland session. This file
is the implementation backlog; completed implementation/check items are supported
by [the verification record](verification.md).
The [spec](spec.md) owns behavior.

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

Add page translation.

MDict, ZIM/Wikipedia, Arabic rule grammar, more dialect coverage, commercial
source catalogs, Windows/macOS adapters, and specialized FST indexing remain
outside the initial implementation. Revisit indexing after measured need.

## Implementation sequence

The approved product contract is in [spec](spec.md). Dependencies flow from
shared snapshots/configuration into dictionary/model services, and into the
Linux adapter; the compositor bridge contains no language processing.

1. Preserve original notes, establish the Rust workspace/contracts, replace
   FST-first assumptions with SQLite, add licenses and balanced evaluation cases.
2. Prove the version-matched bridge in a nested compositor: focus/keymap/device
   events, nonblocking transport, capture leases, disconnect/hang recovery.
3. Deliver live writing: projected typing, asynchronous English checking, latest
   model requests, stale-response rejection, verified AT-SPI edits and Copy.
4. Deliver reading: open-pack installation, StarDict/Kaikki import, selection
   lookup, model translation and focused bilingual search.
5. Complete configurable independent category surfaces, coordinated viewport
   placement, native local media, settings, CLI and Arch packaging.
6. Run formatting, Clippy, Rust tests, CMake/CTest, nested integration and a
   release build. Record evidence rather than marking guessed success.
7. Before release: test actual daily applications, assess 25 reading and 25
   writing cases, reach 40 useful results and complete a one-week personal trial.

No main-session plugin load, user compositor configuration change, cloud
credential use or publication is part of the automated test harness. The broad
capture policy is best effort; opaque secure fields cannot be guaranteed safe.

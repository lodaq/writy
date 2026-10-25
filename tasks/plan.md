# Writy implementation sequence

The approved product contract is in ../docs/spec.md. Dependencies flow from
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

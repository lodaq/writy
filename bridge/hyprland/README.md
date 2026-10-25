# Hyprland input bridge

This optional C++ plugin observes Hyprland's keyboard devices and sends events to
the Rust daemon. It does not grab devices, replay keys, decode text, run models,
or consume any key. Each category HUD remains a separate GTK layer surface in
the Rust application.

It is currently built and runtime tested against **Hyprland 0.56.2** and
**hyprutils 0.14.2**. Plugins share C++ objects with the compositor; rebuild after
Hyprland/toolchain upgrades. Initialization refuses a mismatched full ABI hash.
The old `registerCallbackDynamic` API is inert in this version. This bridge uses
`Event::bus()` and per-device `IKeyboard` signal listeners.

## Build and verify

Dependencies: Hyprland development headers, C++26 compiler, CMake, pkgconf and
nlohmann-json. Build with the compiler used for the installed compositor.

```sh
cmake -S bridge/hyprland -B bridge/hyprland/build -DCMAKE_BUILD_TYPE=RelWithDebInfo -DCMAKE_INSTALL_LIBDIR=lib
cmake --build bridge/hyprland/build
ctest --test-dir bridge/hyprland/build --output-on-failure
python bridge/hyprland/tests/nested_smoke.py bridge/hyprland/build/writy-hyprland.so
```

`cmake --install bridge/hyprland/build --prefix /usr` installs
`/usr/lib/writy/writy-hyprland.so`. Packaging can set `DESTDIR` normally.

The smoke test launches a new nested compositor, verifies its PID and instance
signature, and loads the plugin only into that child. It creates a synthetic
foot/wtype fixture, checks source keymaps, modifiers, press/release events,
capture-off gating and unload. It never loads the plugin into the main session.
It also verifies authorization expiry when the daemon stops renewing and a fresh
hello/device snapshot after reconnect.
Dependencies for this test are Hyprland, foot, wtype and Python 3.

The complete Rust/GTK integration test is `python tests/desktop/run.py` after
building the daemon. That test verifies the GTK fixture's actual text as well
as content-free daemon status, including delivery during daemon hang and exit.

## Protocol version 1

The Rust daemon owns the Unix stream server at
`$XDG_RUNTIME_DIR/writy/bridge.sock`. The plugin connects as a client and verifies
the server UID with `SO_PEERCRED`. The daemon must similarly authenticate clients.
`WRITY_BRIDGE_SOCKET` overrides the path for isolated tests.

Each record is one JSON object followed by a newline. Every outbound record has
a monotonically increasing `seq` and current `focus_epoch` (unsigned integers).
Receivers must discard projected state on a gap, reconnect, reset or unknown
keymap. Additional record fields may be ignored.

| `type` | Additional fields |
| --- | --- |
| `hello` | `version: 1`, `hyprland_abi: string` |
| `focus` | `app_id: string`, `window_id: string`, `pid: u32`, `locked: bool` |
| `keymap` | `device: string`, `keymap: string` (XKB v1), `name: string`, `wayland_virtual: bool`, `repeat_rate: int`, `repeat_delay: int` |
| `modifiers` | `device: string`, `depressed: u32`, `latched: u32`, `locked: u32`, `group: u32` |
| `key` | `device: string`, `keycode: u32`, `state: 0 or 1`, `time_ms: u32` |
| `reset` | `reason: string` |

`keycode` uses Linux/evdev numbering; add **8** for libxkbcommon. Modifier masks
refer to that device's keymap. Physical and Wayland virtual keyboards have
separate IDs. `wayland_virtual` identifies the Wayland virtual-keyboard protocol;
uinput devices arrive through the ordinary hardware backend and can still be
identified by their device name. Device IDs are valid for one plugin session.
Focus records additionally carry optional `monitor_geometry: [x,y,width,height]`
in logical coordinates. Geometry changes emit a refreshed focus record without
resetting the typing segment. This lets HUDs use current compositor bounds when
GTK retains an older output mode size.
Window IDs are Hyprland hexadecimal window addresses; no titles are transmitted.
Navigation and the acceptance chord are recognized through each device's XKB
keymap rather than assuming physical keycodes. Hyprland bindings for virtual
keyboards with custom keycodes may need `input:resolve_binds_by_sym = true`; the
nested test sets this explicitly. Normal desktop bindings remain user-owned.

Rust sends capture authorization records on the same connection:

```json
{"type":"capture","enabled":true,"focus_epoch":42}
```

Only the current epoch is accepted. Authorization expires after **1500 ms**;
renew at least every **500 ms**. Capture defaults off. Focus, pointer-button
presses, navigation, Ctrl/Alt/Super shortcuts, layout/keymap changes, newly
discovered devices, locking and config reload revoke it, increment the epoch,
and emit `reset` followed by `focus`. Keymap/modifier/focus metadata continue
while capture is off. Backspace resets the projected segment. A locked session always
rejects authorization and suppresses key export.

## Failure and privacy behavior

All sockets are nonblocking. Outbound buffering is capped at 2 MiB and flushing
is bounded to 64 KiB per call. A lost connection or overflow drops the queued
session, disables capture and reconnects with a fresh hello/reset and complete
device metadata. Control records are capped at 4096 bytes. The compositor never
waits for Rust. Unloading cancels the timer and releases every listener.

Raw keycodes reveal typed content even before decoding. Capture authorization is
therefore a privacy decision, not merely a performance switch. Rust must exclude
known secure/blocked fields and default terminals to explicit prose mode. AT-SPI
signals are asynchronous: an authorization lease cannot guarantee exclusion of
unknown password widgets or an in-surface focus change not yet reported. Opaque
GUI capture follows the user's selected best-effort policy; it is not a universal
secure-input guarantee. No text or keycodes are logged by the plugin.

The new-device/keymap authorization boundary may discard early events until
Rust reauthorizes; the next segment begins from the events actually received.
Client-side key repeat, IME composition, paste, undo and host autocomplete cannot
be reconstructed authoritatively from these events. Live projection must reset
on uncertainty and direct edits must use separately verified application text.

## Verified evidence

On 2026-10-24: CMake and CTest passed; a fresh nested Hyprland 0.56.2 loaded the
ABI-matched plugin, emitted hello/device/focus/press/release records, suppressed
key records after capture revocation and remained responsive after unload.
This is an integration proof, not an exhaustive compatibility certification.

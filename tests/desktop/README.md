# Nested desktop verification

Build Writy and the bridge, then run:

```sh
cargo build --workspace
cmake -S bridge/hyprland -B bridge/hyprland/build -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build bridge/hyprland/build
python tests/desktop/run.py
```

The test needs Hyprland 0.56.2, wtype, a C compiler, GTK4 development headers,
pkgconf and Python 3. An existing Wayland session hosts the nested compositor.
An optional `--screenshot /absolute/path.png` uses grim against the nested socket
only. `--binary` and `--plugin` accept alternative local build artifacts.

The harness uses a private 0700 temporary runtime/config/data/cache directory.
It verifies the nested compositor's PID and uses that instance for every
hyprctl command. It loads no plugin and launches no Writy window in the parent
desktop. It enables PRIMARY-selection watching and points offline models at
an unavailable loopback port, so synthetic tests do not trigger model downloads,
external requests or GPU inference.

The independent GTK fixture writes only synthetic prose to a test file. Its
password entry never writes that output. Assertions cover live projected input,
Harper findings, the real acceptance binding and verified AT-SPI correction,
automatic PRIMARY lookup, dictionary import/copy, focused search, coordinated
HUD placement, pointer hover, Print-key retention, a synthetic Flameshot focus
transition (including capture exclusion and layer ordering), known-password exclusion, mixed English/Arabic input, ordinary
key delivery with the daemon running/stopped/terminated, and plugin unload.
Only counts, booleans and status are printed. Temporary text and logs are
removed when the run ends, unless `--keep-temp` is requested for diagnostics.
The nested input configuration resolves bindings by symbol so wtype's custom
virtual keycodes trigger the same acceptance action.

Use `--with-model` to additionally exercise the installed offline Ollama model
with synthetic fixture text and wait for generated insert-mode lexical details.
This explicitly enables local inference and extends the typing hold to forty
seconds; the default test keeps models disabled. Both modes assert one bounded reading HUD. The native gallery separately verifies
all seven sections, natural text height, scrolling, expansion and reduced motion.


Run the isolated native component gallery with:

```sh
python tests/desktop/gallery.py --output /tmp/writy-ui
```

It uses a private nested Wayland compositor and synthetic bilingual data. It does
not run the Writy backend, load the compositor bridge, capture desktop content,
or contact a configured model server.

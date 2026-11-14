# Nested desktop verification

Build Writy and the bridge, then run:

```sh
cargo build --workspace
cmake -S bridge/hyprland -B bridge/hyprland/build -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build bridge/hyprland/build
python tests/desktop/run.py
```

The test needs Hyprland 0.56.2, wtype, a C compiler, GTK4 development headers,
pkgconf and Python 3. An existing Wayland session hosts the nested compositor. This can open a visible
window and disrupt the parent session: do not use this harness unchanged when
headless or non-disruptive testing is required. It also inherits the session bus;
private XDG paths alone do not isolate accessibility.
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

It uses Xvfb with software GTK rendering and synthetic bilingual data, inside a
Bubblewrap sandbox with private network, display sockets, devices, home/runtime,
and a fresh D-Bus session. It requires `bwrap`, `dbus-run-session`, `xvfb-run`,
and the installed build dependencies. No system packages are installed by the script.
The build is offline, locked, and limited to two jobs; the compiled test executable
runs inside the sandbox with the source mounted read-only. Only the private fixture
and requested screenshot output directory are writable host mounts.

The gallery does not run the Writy backend, load the compositor bridge, capture
desktop content, contact a configured model server, or prove AT-SPI/Wayland marker
positioning. GLX is disabled to avoid loading GPU drivers in device isolation.
Portal integration is disabled; dictionary and UI tests use only local files.

## Local image input

```sh
python tests/desktop/image_input.py
```

Requires `xvfb-run`, `dbus-run-session`, Tesseract and its English/Arabic data.
Runs GTK on an isolated X11 display and a private session bus. Clipboard images
and OCR fixtures are synthetic. Stub `slurp`/`grim` executables test cancellation
and completed capture without reading the real desktop; Tesseract is real.
Checks cover mixed English/Arabic recognition, ordinary text paste, image paste,
selection replacement, edits around moving badges, queued images, deleted
in-flight badges, invalid/no-text images, Retry/Remove, process output limits and
timeouts. This does not establish live Wayland region-selection behavior or OCR
quality across arbitrary screenshots.

The PNG fixtures contain only generated text, rendered with Pillow and DejaVu
Sans: `Hello world` and a second mixed-language fixture adding `مرحبا بالعالم`.

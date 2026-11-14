# Writy

![Status: Under development](https://img.shields.io/badge/status-under_development-orange)

Writy is a personal English–Arabic reading and writing assistant for
Arch Linux with Hyprland. It combines dictionary lookup, sentence translation,
English proofreading, and suggested phrasing in a compact, scrollable HUD with
dedicated sections.

This is an initial implementation under active development. Build checks and
mock-provider tests do not establish that ambient capture, caret placement, or
replacement work across applications. The isolated GTK/Hyprland fixture has passed live capture, recognized-password exclusion, verified correction, and daemon failure recovery; this does not establish coverage in other applications. The compatibility trial and human
usefulness evaluation are still pending; see [checklist](docs/checklist.md).

## Desktop workspace

The native GTK interface includes a manual search window, tabbed floating reading/writing panels, and settings for privacy, models, dictionaries, and panel visibility. Dictionary management lists installed sources and supports catalogue downloads and imports. Lookups show source attribution; models and dictionaries are separate installations.

The **Suggestions** tab offers local corrections, dictionary synonyms for words, and model-generated corrections or alternative phrasing for words and phrases. Choices include Apply, Copy, and Dismiss. Search Apply updates the input; floating Apply requires verified text and rejects stale suggestions. The Suggestions visibility switch is in Settings → Text assistant and preserves its saved preference. Search has no separate lookup or Copy result buttons; typing runs the work automatically, and Enter still copies the selected tab's result.

The HUD can switch between compact and fullscreen views. Panel content scrolls independently, and the manual search window provides a focusable path for keyboard use. See [interface design](docs/interface-design.md) for the current behavior and validation procedure.

## Intended workflow

- **Read:** Select English or Arabic text for a dictionary entry or sentence
  translation; use `Alt+G` to open or close the focused lookup window. Search translates automatically between English and Arabic as you type, keeps Translation available in either language, and preserves your selected tab.
- **Write:** Keep every enabled HUD section available while typing. Local word lookup appears
  first; labeled AI details and translation follow after a sentence ends with a dot (`.`). English
  corrections run locally, and phrasing starts at two words by default. Existing text is not reconstructed after
  arbitrary edits; uncertain navigation or focus changes reset the buffer.
- **Accept:** `Ctrl+Alt+Enter` applies a suggestion only when its text and target
  are verified. Otherwise, use Copy and paste it yourself.
- **Control:** Pause automatic assistance without disabling manual tools; terminal writing requires
  temporary permission and resets when focus changes.

When a cloud endpoint and credentials are configured, enabled applications can
send text automatically for translation and rewriting. Local dictionaries and
local models provide the offline path. `offline = true` disables all online translation and cloud use.

Settings → Translation offers Google, Microsoft Translator (Bing), Yandex and Apertium
for selection and search translation through direct HTTP APIs. Set
`[model] translation_engine` to `"google"`, `"bing"`, `"yandex"` or `"apertium"`,
or use the engine picker. The default `"model"` preserves existing routing.
Google, Bing and Yandex work without API keys: select the engine and click
Save settings. Turn off Privacy → Offline only to permit online requests.
These keyless web/client interfaces are unofficial and may change, rate-limit
requests, or reject them. Google keyless accepts up to 5000 characters and Bing
up to 1000 UTF-16 units; longer selections use local Ollama with a notice.
Yandex keyless accepts Writy's normal input limit. See the
[keyless checks and implementation notes](docs/keyless-translation.md).

Optional official API keys can be entered under each provider's expandable
section. Saved keys take precedence over environment variables; either selects
the official API instead of the keyless interface. Clear the saved key and remove
its environment variable to return to keyless mode. Keys are stored as plain text
in config.toml with owner-only read/write permissions.

- Google: optional `WRITY_GOOGLE_API_KEY` for Cloud Translation Basic.
- Bing: optional `WRITY_BING_API_KEY` for Azure Translator; set `WRITY_BING_REGION`
  when required by the Azure resource. The region is unused in keyless mode.
- Yandex: optional `WRITY_YANDEX_API_KEY`, a service-account API key with translation access.
- Apertium: no key; its public APy API currently has no English–Arabic pair.

Official APIs may require billing setup. All online translation sends selected
or searched text to the chosen service. Failures fall back to local Ollama with a
notice; offline mode skips all online engines. Live writing assistance continues
using language models. No external translation command is needed.

API references: [Google](https://docs.cloud.google.com/translate/docs/reference/rest/v2/translate),
[Microsoft](https://learn.microsoft.com/en-us/azure/ai-services/translator/text-translation/reference/authentication),
[Yandex](https://aistudio.yandex.ru/en/docs/translate/api-ref/Translation/translate),
[Apertium](https://wiki.apertium.org/wiki/Apertium-apy).

Password-role detection and app blocklists are best effort. Opaque fields and
hidden terminal password prompts may not identify themselves. Prose mode does
not distinguish terminal prose from commands or secrets. Pause Writy before
entering sensitive text in a context it cannot identify.

## Build on Arch Linux

```sh
sudo pacman -S --needed base-devel rust cmake ninja pkgconf hyprland nlohmann-json \
  gtk4 gtk4-layer-shell libxkbcommon at-spi2-core wl-clipboard
cargo build --release --locked
cmake -S bridge/hyprland -B build/bridge -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_INSTALL_LIBDIR=lib
cmake --build build/bridge
ctest --test-dir build/bridge --output-on-failure
```

`target/release/writy` is the application;
`build/bridge/writy-hyprland.so` is the compositor bridge. The bridge must be
built against the same Hyprland version/build as the compositor that loads it.
Rebuild it after a Hyprland update. Begin bridge testing in a nested compositor;
see [the bridge README](bridge/hyprland/README.md).

Optional services: [Ollama](https://docs.ollama.com/) for local generation and
`mpv` for dictionary audio. Models and dictionaries are installed separately;
model quality and speed depend on the selected model and hardware.

For a local development run, start
`GDK_BACKEND=wayland target/release/writy daemon` in the graphical session. Run commands from another terminal using the same binary. Installing
[the Arch package recipe](packaging/arch/README.md) puts `writy` on `PATH` and
installs the optional user service and sample bindings. No input-group or
`uinput` permissions are required by the compositor bridge.

The main window opens fullscreen. Input and results share the workspace equally,
side by side on wide windows and stacked on windows narrower than about 1000 pixels.

## Screenshot and image input

Run `writy screenshot` to select a screen area and open a fresh search with its
recognized text. The sample Hyprland bindings use **Ctrl+Alt+O**. Escape cancels
selection and preserves your existing input. The shortcut also starts the daemon
when necessary.

Paste an image with **Ctrl+V** in the search input. A loading badge appears at the
cursor, replacing selected text when present; surrounding words stay intact.
Keep typing or paste more images while extraction runs. Each result replaces
its own badge; deleting a badge discards its result. Images are processed in paste
order. Failures and images without text show Retry and Remove; error details are
in the badge's tooltip. If reading the clipboard itself failed, Retry reads the
current clipboard again.

Extraction runs locally using Tesseract with English and Arabic language data.
Images are kept in memory and are not uploaded or saved by Writy. Recognized text
uses the normal lookup/translation flow and existing privacy/provider settings.
Lookup ignores pending and failed badges. Image limits are 16 MiB encoded and
32 million pixels; extracted text is limited to the normal 8192 characters.
Extraction times out after 30 seconds. OCR quality depends on text size, contrast,
font, and layout; edit any recognition errors in the input.

Required Arch packages:

```sh
sudo pacman -S --needed grim slurp tesseract tesseract-data-eng tesseract-data-ara
```

`writy doctor` checks the capture/OCR executables. The extraction badge reports
missing language data. No vision model or cloud image service is required.

## Automatic assistance and settings

Settings provides independent **Show on selection**, **Show on copy**, and
**Show while typing** controls. Selection and typing default on; copy defaults off.
**Dismiss** hides a suggestion until the text changes or you leave the field.

Selecting and then copying the same text reuses the Text assistant and its
analysis while the source context remains valid. Writy's own Copy actions do not
trigger another lookup. Automatic selection/copy watching requires `wl-clipboard`;
missing clipboard access shows a notice and leaves manual lookup available.

**Pause automatic assistance** temporarily stops all automatic triggers and
pending automatic requests. **Resume automatic assistance** restores the saved
choices. Manual search and lookup remain usable while paused. `writy pause`
toggles the same temporary state, independently of the legacy saved `enabled` gate.

## Commands

```sh
writy daemon
writy search                         # toggle the focused lookup window
writy screenshot                     # capture an area and extract its text locally
writy translate                      # alias for search
writy lookup --selection             # current Wayland primary selection
writy lookup 'make a decision'
writy accept                         # accept the current verified suggestion
writy pause                          # pause/resume automatic assistance
writy prose                          # allow assistance in this terminal until focus changes
writy settings                       # choose installed Ollama models and edit configuration
writy models                         # list every installed Ollama tag
writy status
writy dict list
writy dict install --help
writy dict import --help
```

For GUI dictionary setup, open **Settings → Dictionary library**. **Explore**
lists eight sources: both FreeDict directions, English and Arabic Wiktionary,
Open English WordNet, Arramooz Alwaseet, GNU GCIDE, and Princeton WordNet.
Click **Download** once to download, extract, convert, and import in the background.
Downloaded sources appear in **Downloaded**, with persistent on/off switches.
Explore keeps them visible with Download disabled and **Remove** available;
Remove deletes installed entries and Writy's cached download. Switching off keeps
the data. GCIDE includes historical usage; Princeton WordNet overlaps OEWN.
Python 3 is included in the Arch package dependencies. Local-file imports remain
available through `writy dict import`.

The expanded library performs a one-time reset of prior dictionary entries,
preferences, and Writy-managed pack files, including custom imports. Download
sources again from Explore. Application settings and original import files
outside Writy's managed cache are preserved; later downloads are never reset.

Built-in FreeDict packs are imported as bilingual translations. For another
bilingual StarDict source, pass `--bilingual` to `writy dict import`; ordinary
StarDict imports treat the payload as definitions. Supply the source's actual
license and attribution when importing.

See the [dictionary download catalogue](docs/dictionary-sources.md) for English
and Arabic sources, licenses, direct download options, and import commands.
Lookups combine installed sources with attribution; the 50-entry result budget
is shared across matching sources so one large dictionary cannot hide the rest.
The GUI installer automatically converts Open English WordNet's official JSON
release and prepares Wiktionary JSONL. Publisher archives and excluded-record
logs are retained in the pack cache for inspection.

Use `writy --help` and each command's `--help` for the executable interface.
Example [Lua bindings](packaging/hyprland/writy.lua) target current Hyprland;
[legacy bindings](packaging/hyprland/writy.conf) are provided separately. Bindings
are examples to merge into your configuration, not an installer that rewrites it.

## Configuration

Configuration is read from `$XDG_CONFIG_HOME/writy/config.toml`, defaulting to
`~/.config/writy/config.toml`. Start with [the example](packaging/config.toml).
Settings loads searchable lists from Ollama automatically; Refresh reloads the
list from the entered Ollama URL. Choose models, then Save settings. The writing
selector covers rewriting, word details and live translation; the other selector
covers selection/search translation. Configured selections remain available if
temporarily absent from the server.

The core validates debounce/timeout ranges and permits a loopback Ollama URL
and an HTTPS cloud endpoint.

Cloud generation remains unconfigured until you set an endpoint, model, and the
API-key environment variable named by `model.api_key_env`. The cloud endpoint
uses the OpenAI-compatible chat interface. Provider compatibility must be tested
against the chosen service. Keep secrets out of the configuration and repository.
For a user service, use its optional environment file as described in
[packaging](packaging/arch/README.md).

The dictionary database lives under `$XDG_DATA_HOME/writy/`, defaulting to
`~/.local/share/writy/`. Runtime control and bridge sockets live under
`$XDG_RUNTIME_DIR/writy/`. Text buffers remain in application memory and are not
intended as persistent history; this does not guarantee that the operating
system, configured model service, or cloud provider never retains data.

## Project map and validation

- [Local desktop setup](docs/desktop-setup.md): installed paths, shortcuts, updates, and undo.
- [Product specification](docs/spec.md): workflows, boundaries, and acceptance.
- [Architecture](docs/architecture.md): engine, providers, Linux adapter, and HUDs.
- [Implementation and trial checklist](docs/checklist.md): remaining work and proof.
- [Research](docs/research.md): primary sources and claims needing measurement.
- [Glossary](GLOSSARY.md) and [decision records](docs/adr/).
- [Original design notes](docs/archive/design-notes.md): preserved historical
  ideas, including superseded proposals and unsupported performance claims.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
python tests/unit.py
cargo build --release --locked
```

[CI](.github/workflows/ci.yml) runs Rust checks and a native bridge build on Arch.
The 50 [evaluation cases](tests/fixtures/evaluation.json) contain exactly 25
reading and 25 writing scenarios. Their usefulness ratings are deliberately
blank. The release goal is at least 40 useful cases, no stale edits or lost
keystrokes, and a week-long personal trial. That goal is not a measured result.

## License

Writy is licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE), at your
option. Dictionaries, media, models, and dependencies retain their own licenses
and attribution requirements. Only redistribute a dataset after checking its
license; import support does not grant redistribution rights.

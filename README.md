# Writy

Writy is a personal English–Arabic reading and writing assistant for
Arch Linux with Hyprland. It combines dictionary lookup, sentence translation,
English proofreading, and suggested phrasing in a compact, scrollable HUD with
dedicated sections.

This is an initial implementation under active development. Build checks and
mock-provider tests do not establish that ambient capture, caret placement, or
replacement work across applications. The isolated GTK/Hyprland fixture has passed live capture, recognized-password exclusion, verified correction, and daemon failure recovery; this does not establish coverage in other applications. The compatibility trial and human
usefulness evaluation are still pending; see [tasks](tasks/todo.md).

## Intended workflow

- **Read:** Select English or Arabic text for a dictionary entry or sentence
  translation; use `Alt+G` for a focused manual lookup.
- **Write:** Keep every enabled HUD section available while typing. Local word lookup appears
  first; labeled AI details and translation follow after a sentence ends with a dot (`.`). English
  corrections run locally, and phrasing starts at two words by default. Existing text is not reconstructed after
  arbitrary edits; uncertain navigation or focus changes reset the buffer.
- **Accept:** `Ctrl+Alt+Enter` applies a suggestion only when its text and target
  are verified. Otherwise, use Copy and paste it yourself.
- **Control:** Pause assistance globally; terminal writing requires explicit
  prose mode and resets when focus changes.

When a cloud endpoint and credentials are configured, enabled applications can
send text automatically for translation and rewriting. Local dictionaries and
local models provide the offline path. `offline = true` disables cloud use.

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

## Commands

```sh
writy daemon
writy search                         # focused manual lookup
writy translate                      # alias for search
writy lookup --selection             # current Wayland primary selection
writy lookup 'make a decision'
writy accept                         # accept the current verified suggestion
writy pause                          # toggle global pause
writy prose                          # toggle terminal prose for current focus
writy settings                       # choose installed Ollama models and edit configuration
writy models                         # list every installed Ollama tag
writy status
writy dict list
writy dict install --help
writy dict import --help
```

Built-in FreeDict packs are imported as bilingual translations. For another
bilingual StarDict source, pass `--bilingual` to `writy dict import`; ordinary
StarDict imports treat the payload as definitions. Supply the source's actual
license and attribution when importing.

See the [dictionary download catalogue](docs/dictionary-sources.md) for English
and Arabic sources, licenses, direct download options, and import commands.
Lookups combine installed sources with attribution; the 50-entry result budget
is shared across matching sources so one large dictionary cannot hide the rest.
Open English WordNet's official JSON release can be converted with
`python3 tools/wordnet_to_jsonl.py ARCHIVE.zip OUTPUT.jsonl` before importing.

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
- [Implementation and trial checklist](tasks/todo.md): remaining work and proof.
- [Research](docs/research.md): primary sources and claims needing measurement.
- [Glossary](GLOSSARY.md) and [decision records](docs/adr/).
- [Original design notes](docs/archive/design-notes.md): preserved historical
  ideas, including superseded proposals and unsupported performance claims.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
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

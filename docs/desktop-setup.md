# Local desktop installation

Installed on this machine on 2026-10-25, after the ABI-matched bridge passed its
nested compositor test. This is a local development installation, separate from
the Arch package. It needs no root access.

## Installed files

- `~/.local/bin/writy`: symlink to this checkout’s `target/release/writy`, created with the user’s `install_local_app` function; on the existing PATH.
- `~/.local/lib/writy/writy-hyprland.so`: bridge matched to Hyprland 0.56.2.
- `~/.config/systemd/user/writy.service`: user service using that executable,
  explicitly selecting GTK's Wayland backend.
- `~/.config/hypr/hyprland.lua`: marked `BEGIN WRITY INTEGRATION` section with
  shortcuts, startup service command, bridge load, an animation-free HUD layer rule,
  and a floating/centered window rule for search and settings.

The Hyprland config is an existing symlink into the user's dotfiles. Existing
content was preserved. A pre-install backup is under
`~/.local/state/writy/setup-backups/20261024T215646Z/` (UTC timestamp).
The service starts from Hyprland's startup callback; it does not need separate
`systemctl enable` activation. Avoid starting a second manual daemon.

The default Ollama models are installed. Dictionary downloads are managed in
Settings → Dictionary library. Cloud assistance is unconfigured. Local generation requires Ollama to be running.

## Everyday controls

| Shortcut | Action |
|---|---|
| Alt+G | Open focused search/editor |
| Ctrl+Alt+G | Look up the current selection |
| Ctrl+Alt+O | Capture a screen area and extract its text (new sample binding) |
| Ctrl+Alt+Enter | Apply the current verified suggestion |
| Ctrl+Alt+W | Pause/resume capture |
| Ctrl+Alt+P | Toggle terminal prose for the current focus |
| Ctrl+Alt+S | Open settings |

The floating writing assistant has scrollable section tabs (Translation, Writing,
Definition, Collocations, Examples, Synonyms & antonyms, and Word family).
Use the tab-strip arrows or Shift + scroll up/down to change sections. Each tab
has a fixed scroll area and retains its reading position. Drag the header to move
the panel; its position persists for the session. The fullscreen button fills the
current monitor and restores the compact panel on the next click. Drag across
result text to select it; Arabic content is aligned to the right.

Alt+G fills the input from the current text selection, or from the clipboard
when there is no selected text, then translates that prefilled text automatically.
This also works when the shortcut starts the daemon. Multi-line text is preserved.

Settings → Dictionaries lists all eight
sources: both FreeDict directions, English and Arabic Wiktionary, Open English
WordNet, Arramooz Alwaseet, GNU GCIDE, and Princeton WordNet 3.0. GCIDE includes
historical English usage; Princeton WordNet overlaps Open English WordNet.
Publisher and licence links remain available beside each source.

Click **Download** once to download, extract, convert, and import in the
background. The same row then shows an enabled on/off switch. Switches save immediately and affect lookups;
turning one off keeps its data. Installed sources show an active **Remove** button. Remove immediately deletes the
source's entries, preference, and managed cached download. Download becomes
available again. Progress and failures are shown without blocking the desktop.

The first library access after this update resets all old dictionary sources,
including custom imports, their toggle preferences, and Writy's pack cache.
Download them again from Explore. This reset is recorded once in the dictionary
database, so subsequent starts preserve new downloads. Application settings and
original files outside Writy's cache remain untouched. Failed cleanup reports an
error and can be retried without discarding installed entries.

Local StarDict `.ifo` and prepared Kaikki `.jsonl` files can still be imported with
`writy dict import`. Catalogue installation needs no terminal. Python 3 is included
in the packaged runtime dependencies. Publisher archives, licence files, and
preparation records are retained in Writy's cache until the source is removed.

Useful commands:

```sh
writy status
writy search
writy models
writy settings
writy dismiss
systemctl --user status writy.service
journalctl --user -u writy.service -b
```

Terminals remain excluded until prose mode is explicitly enabled. Changing focus
resets that opt-in. Unknown secure fields remain subject to the documented
best-effort privacy policy.

## Rebuild and update

Rebuild the bridge whenever Hyprland changes and repeat the nested test before
loading it into the working session. Follow [the bridge guide](../bridge/hyprland/README.md).
Build artifacts in the repository are not automatically deployed to `~/.local`.
Unload the installed bridge before replacing its library; stop the service before
replacing its executable. The command symlink now follows this checkout’s release build:

```sh
cargo build --release --locked
systemctl --user restart writy.service
writy status
```

The local command was registered using:

```sh
source ~/dotfiles/zsh/.oh-my-zsh/custom/functions.zsh
install_local_app -p /home/sebaq/repos/writy/target/release/writy
```

The function does not overwrite an existing command. During this update the old
executable and Hyprland config were preserved in
`~/.local/state/writy/setup-backups/20261025T023740Z/` before installing the link.
HUDs use Wayland overlay layer 3; the floating window rule applies to search and
settings, since layer surfaces are already independent of tiling.

## Stop or remove the integration

```sh
systemctl --user stop writy.service
hyprctl plugin unload "$HOME/.local/lib/writy/writy-hyprland.so"
```

To stop future automatic activation, remove only the marked Writy section from
`~/.config/hypr/hyprland.lua`, then run `hyprctl reload`. This preserves other
configuration changes made before or after the installation. The installed
binary, bridge, service and dictionary data can be removed separately.

## Translation provider setup

Open Settings → Translation. Choose Google, Bing or Yandex and click Save
settings; no key is needed. Turn off General → Offline only to allow online
requests. Changes apply without restarting. Keyless services are unofficial and
can fail or rate-limit requests; local Ollama remains the fallback.

For an optional official API, expand its credential section and configure it:

- Google: create a Cloud project, enable billing and Cloud Translation API,
  create an API key under APIs & Services → Credentials, and restrict it to
  Cloud Translation API. Paste it into Google API key.
- Microsoft (Bing): create a public Azure Translator resource, open Resource
  Management → Keys and Endpoint, and paste Key 1. Copy the region into
  Microsoft region if using a regional resource; leave blank for Global.
- Yandex: create a service account in your folder, grant ai.translate.user,
  create an API key with yc.ai.translate.execute scope, and paste the key.
- Apertium: no credentials, but its public API currently has no English–Arabic
  pair, so requests fall back to Ollama.

The setup links in each provider section open its official instructions. Saved
keys are plain text in the owner-only configuration file. Keep them out of
screenshots and shared config copies. Empty key fields use environment variables,
then keyless translation when no key is set.

After a Hyprland upgrade, rebuild the bridge and run its nested smoke test before
loading it. With no bridge loaded, `writy status` reports bridge_connected=false;
automatic typing and selection cannot identify the focused app and remain inactive.

### Image-input update

Rebuild and restart the daemon to load image support. Add the new sample
`Ctrl+Alt+O` binding (`writy screenshot`) to your Hyprland configuration if desired;
repository changes do not rewrite your live bindings. Install `slurp` alongside
`grim`, `tesseract`, `tesseract-data-eng`, and `tesseract-data-ara`. Paste images
with Ctrl+V in the search editor. See the [image-input workflow](../README.md#screenshot-and-image-input).

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

Both FreeDict packs and the default Ollama models are already installed. Cloud
assistance is unconfigured. Local generation requires Ollama to be running.

## Everyday controls

| Shortcut | Action |
|---|---|
| Alt+G | Open focused search/editor |
| Ctrl+Alt+G | Look up the current selection |
| Ctrl+Alt+Enter | Apply the current verified suggestion |
| Ctrl+Alt+W | Pause/resume capture |
| Ctrl+Alt+P | Toggle terminal prose for the current focus |
| Ctrl+Alt+S | Open settings |

The floating writing assistant has scrollable section tabs (Translation, Writing,
Definition, Collocations, Examples, Synonyms & antonyms, and Word family).
Use the tab-strip arrows to reach additional sections. Drag across result text
to select it; Arabic content is aligned to the right.

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

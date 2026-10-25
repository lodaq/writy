# Arch development package

`PKGBUILD` is a development recipe for the committed upstream tree. It is not an
AUR publication or a report of a successful clean-chroot build. Local uncommitted
changes are not included when makepkg clones its source URL. For this checkout,
use the source build instructions until the intended revision is published.

From a directory containing the recipe, run `makepkg -si` as a normal user.
Review its source revision first. The package installs `writy`, the compositor
plugin, a user service, and example configuration/bindings. Rebuild the plugin
when Hyprland changes; a matching compiler build is required for native plugins.
No service is enabled and no compositor configuration is edited by packaging.

## User session

After installation, copy `/usr/share/doc/writy/config.toml` to
`~/.config/writy/config.toml` if you want a persistent configuration. Merge the
sample Lua bindings into your Hyprland configuration. Start the service only
inside the graphical session so the user manager has the right environment:

```sh
systemctl --user import-environment WAYLAND_DISPLAY HYPRLAND_INSTANCE_SIGNATURE XDG_CURRENT_DESKTOP
systemctl --user start writy.service
writy status
```

The supplied sample starts the service from Hyprland's start callback. Use either
that callback or your graphical-session service integration; avoid duplicate
manual daemons. Stop with `systemctl --user stop writy.service`.

For cloud use, set `cloud_url` and `cloud_model` in configuration and provide the
named API-key variable to the process. The user service optionally reads
`~/.config/writy/environment`, containing systemd environment assignments such as
`WRITY_API_KEY=...`. Create this file locally with mode `0600`; never commit it.
Read [systemd environment-file syntax](https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html#EnvironmentFile=)
for quoting rules. Restart the service after changing its environment/config.
With a cloud provider configured, eligible typed/selected text can be sent
automatically. Set `offline = true` to suppress cloud requests.

The bridge is deliberately not auto-loaded by the recipe. Follow
[the bridge's nested-session guide](../../bridge/hyprland/README.md) before loading
`/usr/lib/writy/writy-hyprland.so` into the session you depend on.

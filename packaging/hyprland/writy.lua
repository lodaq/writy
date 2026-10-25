-- Merge into your Hyprland Lua config. Commands run outside the compositor loop.
-- Start via the user service OR `writy daemon`, not both.
hl.on("hyprland.start", function()
    hl.exec_cmd("systemctl --user import-environment WAYLAND_DISPLAY HYPRLAND_INSTANCE_SIGNATURE XDG_CURRENT_DESKTOP; systemctl --user start writy.service")
end)

hl.bind("ALT + G", hl.dsp.exec_cmd("writy search"))
hl.bind("CTRL + ALT + G", hl.dsp.exec_cmd("writy lookup --selection"))
hl.bind("CTRL + ALT + Return", hl.dsp.exec_cmd("writy accept"))
hl.bind("CTRL + ALT + W", hl.dsp.exec_cmd("writy pause"))
hl.bind("CTRL + ALT + P", hl.dsp.exec_cmd("writy prose"))
hl.bind("CTRL + ALT + S", hl.dsp.exec_cmd("writy settings"))

hl.layer_rule({
    name = "writy-instant",
    match = { namespace = "^writy.*$" },
    no_anim = true,
})

-- Load only after the bridge passes nested-session testing against this build:
-- hl.on("hyprland.start", function()
--     hl.exec_cmd("hyprctl plugin load /usr/lib/writy/writy-hyprland.so")
-- end)

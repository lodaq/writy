#!/usr/bin/env python3
"""Render synthetic GTK components in a private nested Wayland desktop."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=Path("/tmp/writy-ui"))
    args = parser.parse_args()
    # Build before starting the compositor, so package-cache contention cannot
    # consume the bounded graphical test timeout.
    subprocess.run(["cargo", "test", "--offline", "-p", "writy-linux",
                    "--no-run"], cwd=ROOT, check=True, timeout=180)
    parent_socket = Path(os.environ["WAYLAND_DISPLAY"])
    if not parent_socket.is_absolute():
        parent_socket = Path(os.environ["XDG_RUNTIME_DIR"]) / parent_socket
    with tempfile.TemporaryDirectory(prefix="wg-") as temporary:
        base = Path(temporary)
        (base / "config/writy").mkdir(parents=True)
        (base / "config/writy/config.toml").write_text(
            'offline = true\n[model]\nollama_url = "http://127.0.0.1:9"\ntimeout_secs = 1\n')
        config = base / "hyprland.conf"
        config.write_text("""monitor = , 960x1100@60, auto, 1
animations {
 enabled = false
}
misc {
 disable_hyprland_logo = true
 disable_splash_rendering = true
}
windowrule = float on, match:class ^io.github.lodaq.writy.visual-test$
""")
        env = dict(os.environ, XDG_RUNTIME_DIR=str(base),
                   XDG_CONFIG_HOME=str(base / "config"), XDG_DATA_HOME=str(base / "data"),
                   XDG_CACHE_HOME=str(base / "cache"), WAYLAND_DISPLAY=str(parent_socket),
                   HYPRLAND_NO_SD_VARS="1", HYPRLAND_NO_SD_NOTIFY="1",
                   HYPRLAND_NO_CRASHREPORTER="1", AQ_DRM_DEVICES="/dev/null")
        with (base / "compositor.log").open("w+") as log:
            compositor = subprocess.Popen(["Hyprland", "--config", str(config)],
                                          env=env, stdout=log, stderr=log, start_new_session=True)
            try:
                deadline = time.monotonic() + 15
                instance = None
                while time.monotonic() < deadline:
                    if compositor.poll() is not None:
                        log.seek(0)
                        raise RuntimeError(log.read()[-2000:])
                    inventory = subprocess.run(["hyprctl", "-j", "instances"], env=env,
                                               capture_output=True, text=True, timeout=3)
                    if inventory.returncode == 0:
                        try:
                            instance = next((i for i in json.loads(inventory.stdout)
                                             if i["pid"] == compositor.pid), None)
                        except json.JSONDecodeError:
                            pass
                    if instance:
                        break
                    time.sleep(.1)
                if not instance:
                    raise RuntimeError("Nested gallery did not start")
                child = dict(env, WAYLAND_DISPLAY=str(base / instance["wl_socket"]),
                             HYPRLAND_INSTANCE_SIGNATURE=instance["instance"],
                             GDK_BACKEND="wayland", GTK_A11Y="none", GSK_RENDERER="cairo",
                             WRITY_UI_SNAPSHOTS=str(args.output.resolve()))
                errors = subprocess.check_output(
                    ["hyprctl", "--instance", instance["instance"], "configerrors"],
                    env=child, text=True, timeout=4).strip()
                if errors:
                    raise RuntimeError(errors)
                subprocess.run(["cargo", "test", "--offline", "-p", "writy-linux", "native_component_gallery",
                                "--", "--ignored", "--test-threads=1", "--nocapture"],
                               cwd=ROOT, env=child, check=True, timeout=60)
            finally:
                if compositor.poll() is None:
                    os.killpg(compositor.pid, signal.SIGTERM)
                    try:
                        compositor.wait(timeout=3)
                    except subprocess.TimeoutExpired:
                        os.killpg(compositor.pid, signal.SIGKILL)
                        compositor.wait(timeout=3)


if __name__ == "__main__":
    main()

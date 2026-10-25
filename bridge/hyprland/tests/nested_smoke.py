#!/usr/bin/env python3
"""Load the bridge only into a fresh nested Hyprland, then check real events.

No commands target the caller's compositor. Only synthetic fixture text is used.
"""
import argparse
import json
import os
from pathlib import Path
import select
import signal
import socket
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("plugin", type=Path)
    args = parser.parse_args()
    plugin = args.plugin.resolve(strict=True)
    runtime = Path(os.environ["XDG_RUNTIME_DIR"])
    before = set((runtime / "hypr").iterdir())
    children = []
    with tempfile.TemporaryDirectory(prefix="writy-nested-", dir=runtime) as directory:
        directory = Path(directory)
        server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        server.bind(str(directory / "bridge.sock"))
        server.listen()
        server.settimeout(15)
        config = directory / "hyprland.conf"
        config.write_text("""monitor = , 800x600@60, auto, 1
animations {
  enabled = false
}
misc {
  disable_hyprland_logo = true
  disable_splash_rendering = true
}
input {
  kb_layout = us,ara
}
""")
        environment = dict(os.environ, HYPRLAND_NO_SD_VARS="1", HYPRLAND_NO_SD_NOTIFY="1",
                           HYPRLAND_NO_CRASHREPORTER="1", AQ_DRM_DEVICES="/dev/null",
                           WRITY_BRIDGE_SOCKET=str(directory / "bridge.sock"))
        log = (directory / "nested.log").open("w+")
        compositor = subprocess.Popen(["Hyprland", "--config", str(config)], env=environment,
                                      stdout=log, stderr=log, start_new_session=True)
        children.append(compositor)
        peer = None
        try:
            signature = None
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline:
                if compositor.poll() is not None:
                    log.seek(0)
                    raise RuntimeError("nested compositor failed: " + log.read()[-5000:])
                candidates = set((runtime / "hypr").iterdir()) - before
                for candidate in candidates:
                    if (candidate / ".socket.sock").exists():
                        output = subprocess.run(["hyprctl", "--instance", candidate.name, "-j", "instances"],
                                                capture_output=True, text=True).stdout
                        try:
                            matches = [item for item in json.loads(output) if item.get("pid") == compositor.pid]
                        except json.JSONDecodeError:
                            matches = []
                        if matches:
                            signature = candidate.name
                            break
                if signature:
                    break
                time.sleep(0.1)
            assert signature and signature != os.environ.get("HYPRLAND_INSTANCE_SIGNATURE"), "no verified nested instance"
            def ctl(*command):
                return subprocess.run(["hyprctl", "--instance", signature, *command],
                                      capture_output=True, text=True, check=True, timeout=5).stdout
            # The only load operation follows the verified child PID/signature check above.
            result = ctl("plugin", "load", str(plugin))
            assert "ok" in result.lower(), result
            peer, _ = server.accept()
            peer.setblocking(False)
            records, pending = [], b""
            epoch = None
            def pump(seconds, authorize=True):
                nonlocal pending, epoch
                until = time.monotonic() + seconds
                while time.monotonic() < until:
                    if select.select([peer], [], [], 0.03)[0]:
                        data = peer.recv(262144)
                        assert data, "bridge disconnected"
                        pending += data
                        while b"\n" in pending:
                            line, pending = pending.split(b"\n", 1)
                            record = json.loads(line)
                            records.append(record)
                            epoch = record["focus_epoch"]
                    if authorize and epoch is not None:
                        peer.sendall((json.dumps({"type": "capture", "enabled": True, "focus_epoch": epoch}) + "\n").encode())
            pump(0.2)
            assert records[0]["type"] == "hello" and records[0]["version"] == 1
            # Find the nested Wayland socket from its own instance information.
            instances = json.loads(ctl("-j", "instances"))
            instance = next(item for item in instances if item["pid"] == compositor.pid)
            nested_env = dict(environment, WAYLAND_DISPLAY=instance["wl_socket"], HYPRLAND_INSTANCE_SIGNATURE=signature)
            foot = subprocess.Popen(["foot", "--app-id=writy-bridge-fixture", "sh", "-c", "cat"],
                                    env=nested_env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            children.append(foot)
            pump(0.6)
            # wtype waits after creating its virtual keyboard, allowing source registration + policy ack.
            typing = subprocess.Popen(["wtype", "-s", "500", "writyfixture"], env=nested_env,
                                      stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            children.append(typing)
            pump(1.0)
            assert typing.wait(timeout=3) == 0, typing.stderr.read().decode()
            keys = [record for record in records if record["type"] == "key"]
            assert keys and {record["state"] for record in keys} == {0, 1}, "no press/release events"
            keymaps = {record["device"]: record for record in records if record["type"] == "keymap"}
            assert all(record["device"] in keymaps for record in keys), "key without device keymap"
            assert any(record.get("wayland_virtual") for record in keymaps.values()), "virtual keyboard not identified"
            assert any(record.get("app_id") == "writy-bridge-fixture" for record in records), "no fixture focus"
            assert any(record.get("pid") == foot.pid for record in records if record["type"] == "focus"), "incorrect focus PID"
            sequence = [record["seq"] for record in records]
            assert sequence == sorted(set(sequence)), "non-monotonic sequence"
            pump(0.1)
            pump(1.8, authorize=False)
            assert any(record.get("reason") == "lease-expired" for record in records), "hung daemon lease did not expire"
            # A daemon restart must receive a fresh hello/reset and complete device state.
            peer.close()
            peer, _ = server.accept()
            peer.setblocking(False)
            reconnect_start = len(records)
            pending = b""
            pump(0.2)
            assert records[reconnect_start]["type"] == "hello", "reconnect missing hello"
            assert any(record["type"] == "keymap" for record in records[reconnect_start:]), "reconnect missing keymaps"
            # Revoke the explicit fixture permission; normal input must still reach the client.
            peer.sendall((json.dumps({"type": "capture", "enabled": False, "focus_epoch": epoch}) + "\n").encode())
            pump(0.1, authorize=False)
            count_before = len([record for record in records if record["type"] == "key"])
            disabled = subprocess.Popen(["wtype", "-s", "100", "privatefixture"], env=nested_env)
            children.append(disabled)
            pump(0.5, authorize=False)
            assert disabled.wait(timeout=3) == 0
            assert len([record for record in records if record["type"] == "key"]) == count_before, "capture-off leaked keys"
            assert "ok" in ctl("plugin", "unload", str(plugin)).lower()
            assert json.loads(ctl("-j", "version"))["version"] == "0.56.2", "compositor failed after unload"
            print(json.dumps({"nested": True, "hyprland": "0.56.2", "hello": True,
                              "press_release_records": len(keys), "devices": len(keymaps),
                              "reset_reasons": sorted({record["reason"] for record in records if record["type"] == "reset"}),
                              "focus": True, "capture_off": True, "lease_expiry": True,
                              "reconnect": True, "unload": True}))
        finally:
            if peer:
                peer.close()
            server.close()
            for child in reversed(children):
                if child.poll() is None:
                    child.terminate()
            for child in reversed(children):
                try:
                    child.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()


if __name__ == "__main__":
    main()

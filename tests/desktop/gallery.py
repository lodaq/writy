#!/usr/bin/env python3
"""Render synthetic GTK components in an isolated virtual X display."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=Path("/tmp/writy-ui"))
    args = parser.parse_args()
    build = subprocess.run(["cargo", "test", "--offline", "--locked", "-j", "2",
                            "-p", "writy-linux", "--no-run", "--message-format=json"],
                           cwd=ROOT, check=True, capture_output=True, text=True, timeout=180)
    artifacts = [json.loads(line) for line in build.stdout.splitlines()]
    executable = next(item['executable'] for item in artifacts
                      if item.get('reason') == 'compiler-artifact'
                      and item.get('target', {}).get('name') == 'writy'
                      and item.get('profile', {}).get('test') and item.get('executable'))
    args.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="wg-") as temporary:
        base = Path(temporary)
        (base / "config/writy").mkdir(parents=True)
        (base / "config/writy/config.toml").write_text(
            'offline = true\n[model]\nollama_url = "http://127.0.0.1:9"\ntimeout_secs = 1\n')
        for test in ("native_bilingual_tabs_preserve_selection_without_translate_button",
                     "native_component_gallery"):
            command = ["bwrap", "--unshare-all", "--die-with-parent", "--new-session",
                       "--ro-bind", "/", "/", "--tmpfs", "/home",
                       "--ro-bind", str(ROOT), str(ROOT), "--tmpfs", "/run",
                       "--tmpfs", "/tmp", "--perms", "01777", "--dir", "/tmp/.X11-unix",
                       "--dev", "/dev", "--proc", "/proc",
                       "--bind", str(base), str(base),
                       "--bind", str(args.output.resolve()), "/tmp/output", "--clearenv"]
            private = dict(
                XDG_RUNTIME_DIR=str(base), XDG_CONFIG_HOME=str(base / 'config'),
                XDG_DATA_HOME=str(base / 'data'), XDG_CACHE_HOME=str(base / 'cache'),
                GDK_BACKEND='x11', GTK_A11Y='none', GSK_RENDERER='cairo',
                WRITY_TEST_XVFB='1', PATH='/usr/bin:/bin', HOME=str(base), LANG='C.UTF-8',
                WRITY_UI_SNAPSHOTS='/tmp/output', GDK_DEBUG='no-portals',
                LIBGL_ALWAYS_SOFTWARE='1',
                __EGL_VENDOR_LIBRARY_FILENAMES='/usr/share/glvnd/egl_vendor.d/50_mesa.json')
            for key, value in private.items():
                command.extend(["--setenv", key, value])
            command.extend(["--chdir", str(ROOT), "xvfb-run", "-a",
                            "-s", "-screen 0 1280x1024x24 -extension GLX",
                            "dbus-run-session", "--", executable, test, "--ignored", "--test-threads=1"])
            subprocess.run(command, cwd=ROOT, check=True, timeout=90)


if __name__ == "__main__":
    main()

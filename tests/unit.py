#!/usr/bin/env python3
"""Run unit tests without borrowing the desktop's accessibility bus or runtime."""
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    env = os.environ.copy()
    home = Path.home()
    env.setdefault('CARGO_HOME', str(home / '.cargo'))
    env.setdefault('RUSTUP_HOME', str(home / '.rustup'))
    for key in ('DISPLAY', 'WAYLAND_DISPLAY', 'AT_SPI_BUS_ADDRESS',
                'DBUS_SESSION_BUS_ADDRESS', 'DBUS_STARTER_ADDRESS', 'DBUS_STARTER_BUS_TYPE'):
        env.pop(key, None)
    with tempfile.TemporaryDirectory(prefix='writy-unit-') as temporary:
        env.update(HOME=temporary, XDG_RUNTIME_DIR=temporary,
                   XDG_CONFIG_HOME=temporary+'/config', XDG_DATA_HOME=temporary+'/data',
                   XDG_CACHE_HOME=temporary+'/cache', ATSPI_DBUS_IMPLEMENTATION='dbus-daemon',
                   GIO_USE_VFS='local', GSETTINGS_BACKEND='memory')
        return subprocess.run(['dbus-run-session', '--', 'cargo', 'test', '--offline',
                               '--locked', '-j', '2', '--workspace', '--', '--test-threads=2'],
                              cwd=ROOT, env=env).returncode


if __name__ == '__main__':
    raise SystemExit(main())

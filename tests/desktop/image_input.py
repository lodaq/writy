#!/usr/bin/env python3
"""Exercise local OCR and GTK image paste without accessing the real desktop."""
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]

with tempfile.TemporaryDirectory(prefix='writy-image-test-') as temporary:
    directory = Path(temporary)
    # First selection cancels; the second returns a synthetic rectangle.
    for name, script in {
        'slurp': '#!/bin/sh\nif [ ! -e "$WRITY_CAPTURE_MARKER" ]; then\n'
                 'touch "$WRITY_CAPTURE_MARKER"\nexit 1\nfi\nprintf "0,0 900x140\\n"\n',
        'grim': '#!/bin/sh\ncat "$WRITY_CAPTURE_FIXTURE"\n',
    }.items():
        path = directory / name
        path.write_text(script)
        path.chmod(0o700)
    env = os.environ.copy()
    env.update(PATH=temporary + ':' + env['PATH'], GDK_BACKEND='x11',
               GIO_USE_VFS='local', GSK_RENDERER='cairo', XDG_CURRENT_DESKTOP='',
               XDG_RUNTIME_DIR=temporary, GTK_A11Y='none', GSETTINGS_BACKEND='memory',
               XDG_CONFIG_HOME=temporary + '/config', XDG_DATA_HOME=temporary + '/data',
               XDG_CACHE_HOME=temporary + '/cache',
               WRITY_CAPTURE_MARKER=str(directory / 'selected'),
               WRITY_CAPTURE_FIXTURE=str(ROOT / 'tests/fixtures/ocr-english.png'))
    env.pop('WAYLAND_DISPLAY', None)
    raise SystemExit(subprocess.run([
        'xvfb-run', '-a', 'dbus-run-session', '--', 'cargo', 'test', '--locked',
        '-p', 'writy-linux', 'image_input_runtime', '--', '--ignored', '--nocapture',
        '--test-threads=1',
    ], cwd=ROOT, env=env).returncode)

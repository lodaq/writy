#!/usr/bin/env python3
"""Cold-start the real search action in an isolated compositor with a mock translator.

Requires a debug build, the bridge build, and the desktop fixture dependencies.
Only synthetic selections and a local HTTP server are used.
"""
import http.server
import json
import os
import pathlib
import signal
import subprocess
import threading
import time
root = pathlib.Path(__file__).resolve().parents[2]
seen = []

class Handler(http.server.BaseHTTPRequestHandler):

    def log_message(self, *args):
        pass

    def do_GET(self):
        self.reply({'models': [{'name': 'translategemma:4b', 'details': {'format': 'gguf'}}]})

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        if self.path == '/api/show':
            self.reply({'details': {'format': 'gguf'}})
        else:
            seen.append(body)
            self.reply({'message': {'content': 'ترجمة الاختبار'}, 'done': True})

    def reply(self, value):
        b = json.dumps(value).encode()
        self.send_response(200)
        self.send_header('Content-Length', str(len(b)))
        self.end_headers()
        self.wfile.write(b)
srv = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
threading.Thread(target=srv.serve_forever, daemon=True).start()
runner = subprocess.Popen(['python3', str(root / 'tests/desktop/run.py'), '--fixture-only'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
env = None
search = None
try:
    info = json.loads(runner.stdout.readline())
    base = pathlib.Path(info['runtime'])
    env = dict(os.environ, XDG_RUNTIME_DIR=str(base), XDG_CONFIG_HOME=str(base / 'config'), XDG_DATA_HOME=str(base / 'data'), XDG_CACHE_HOME=str(base / 'cache'), WAYLAND_DISPLAY=info['wayland_display'], HYPRLAND_INSTANCE_SIGNATURE=info['hyprland_instance'], GDK_BACKEND='wayland', GSK_RENDERER='cairo')
    p = base / 'config/writy/config.toml'
    p.write_text(p.read_text().replace('127.0.0.1:9', f'127.0.0.1:{srv.server_port}'))
    subprocess.run(['wl-copy', '--primary', 'Cold startup selection.'], env=env, check=True)
    search = subprocess.Popen([str(root / 'target/debug/writy'), 'search'], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    assert search.wait(timeout=10) == 0, 'Search launcher failed'
    deadline = time.monotonic() + 8
    while not seen and time.monotonic() < deadline:
        time.sleep(0.1)
    assert seen, 'Cold-start Alt+G action did not request a translation of the selection'
    assert 'Cold startup selection.' in json.dumps(seen[0]), seen
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        subprocess.run([str(root / 'target/debug/writy'), 'copy'], env=env, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.2)
        copied = subprocess.run(['wl-paste', '--no-newline'], env=env, capture_output=True, text=True)
        if copied.stdout == 'ترجمة الاختبار':
            break
    else:
        raise AssertionError('The opened app did not return a completed translation')
    print('PASS: no daemon -> PRIMARY selection -> Alt+G search action -> completed translation')
finally:
    if search:
        try:
            os.killpg(search.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    runner.communicate('quit\n', timeout=8)
    srv.shutdown()

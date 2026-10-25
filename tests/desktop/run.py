#!/usr/bin/env python3
"""Exercise the real daemon/bridge/GTK fixture in an isolated nested compositor."""
import argparse
import contextlib
import json
import os
from pathlib import Path
import shlex
import signal
import subprocess
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[2]


def wait_for(check, label, timeout=8):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = check()
        if value:
            return value
        time.sleep(0.05)
    raise AssertionError("Timed out: " + label)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/writy")
    parser.add_argument("--plugin", type=Path, default=ROOT / "bridge/hyprland/build/writy-hyprland.so")
    parser.add_argument("--screenshot", type=Path)
    parser.add_argument("--keep-temp", action="store_true")
    parser.add_argument("--with-model", action="store_true", help="Exercise installed offline Ollama models with synthetic fixture text")
    parser.add_argument("--fixture-only", action="store_true", help="Hold a nested fixture for broker diagnostics; stdin selects prose/password/clear or quit")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=not args.fixture_only)
    plugin = args.plugin.resolve(strict=True)
    original_runtime = Path(os.environ["XDG_RUNTIME_DIR"])
    original_display = Path(os.environ["WAYLAND_DISPLAY"])
    if not original_display.is_absolute():
        original_display = original_runtime / original_display
    processes = []
    # Short pathname keeps Hyprland's long instance socket below sun_path's limit.
    temporary_context = contextlib.nullcontext(tempfile.mkdtemp(prefix="we-")) if args.keep_temp else tempfile.TemporaryDirectory(prefix="we-")
    with temporary_context as temporary:
        base = Path(temporary)
        for name in ["config/writy", "data", "cache"]:
            (base / name).mkdir(parents=True)
        (base / "config/writy/config.toml").write_text("""offline = true
selection_popup = true
[typing]
idle_ms = 10000
min_rephrase_words = 100
[model]
ollama_url = "http://127.0.0.1:9"
timeout_secs = 1
""")
        if args.with_model:
            app_config=base / "config/writy/config.toml"
            app_config.write_text(app_config.read_text().replace("idle_ms = 10000", "idle_ms = 300").replace("min_rephrase_words = 100", "min_rephrase_words = 2").replace("127.0.0.1:9", "127.0.0.1:11434").replace("timeout_secs = 1", "timeout_secs = 30"))
        config = base / "hyprland.conf"
        config.write_text("""monitor = , 900x700@60, auto, 1
animations {
 enabled = false
}
misc {
 disable_hyprland_logo = true
 disable_splash_rendering = true
}
input {
 kb_layout = us,ara
 resolve_binds_by_sym = true
}
""")
        acceptance = base / "accept.sh"
        acceptance.write_text("#!/bin/sh\n" + "\n".join(
            shlex.quote(str(binary)) + " " + command + " >>" + shlex.quote(str(base / "accept.log")) + " 2>&1"
            for command in ("status", "accept", "status")) + "\n")
        acceptance.chmod(0o700)
        with config.open("a") as stream:
            stream.write("\nbind = CTRL ALT, Return, exec, " + shlex.quote(str(acceptance)) + "\n")
        env = dict(os.environ, XDG_RUNTIME_DIR=str(base), XDG_CONFIG_HOME=str(base / "config"),
                   XDG_DATA_HOME=str(base / "data"), XDG_CACHE_HOME=str(base / "cache"),
                   WAYLAND_DISPLAY=str(original_display), HYPRLAND_NO_SD_VARS="1",
                   HYPRLAND_NO_SD_NOTIFY="1", HYPRLAND_NO_CRASHREPORTER="1", AQ_DRM_DEVICES="/dev/null",
                   WRITY_BRIDGE_SOCKET=str(base / "writy/bridge.sock"))
        log = (base / "compositor.log").open("w+")
        compositor = subprocess.Popen(["Hyprland", "--config", str(config)], env=env,
                                      stdout=log, stderr=log, start_new_session=True)
        processes.append(compositor)
        try:
            def find_instance():
                if compositor.poll() is not None:
                    log.seek(0)
                    raise RuntimeError("Nested compositor failed: " + log.read()[-3000:])
                result = subprocess.run(["hyprctl", "-j", "instances"], env=env, capture_output=True, text=True)
                try:
                    return next((item for item in json.loads(result.stdout) if item["pid"] == compositor.pid), None)
                except json.JSONDecodeError:
                    return None
            instance = wait_for(find_instance, "nested Hyprland", 15)
            signature = instance["instance"]
            assert signature != os.environ.get("HYPRLAND_INSTANCE_SIGNATURE")
            nested_env = dict(env, WAYLAND_DISPLAY=str(base / instance["wl_socket"]), HYPRLAND_INSTANCE_SIGNATURE=signature,
                              GDK_BACKEND="wayland", GTK_A11Y="atspi")
            def start_fixture():
                fixture_binary = base / "fixture"
                flags = shlex.split(subprocess.check_output(["pkg-config", "--cflags", "--libs", "gtk4"], text=True))
                subprocess.run(["cc", "-Wall", "-Wextra", str(ROOT / "tests/desktop/fixture.c"), "-o", str(fixture_binary), *flags], check=True)
                output = base / "fixture-text"
                fixture = subprocess.Popen([str(fixture_binary), str(output)], env=nested_env, stdin=subprocess.PIPE,
                                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                processes.append(fixture)
                return fixture, output
            if args.fixture_only:
                fixture, _ = start_fixture()
                print(json.dumps({"fixture_pid": fixture.pid, "runtime": str(base), "wayland_display": nested_env["WAYLAND_DISPLAY"], "hyprland_instance": signature}), flush=True)
                for line in sys.stdin:
                    if line.strip() == "quit":
                        break
                    if line.strip() in {"password", "prose", "clear"}:
                        fixture.stdin.write(line.encode())
                        fixture.stdin.flush()
                return
            def ctl(*command):
                return subprocess.run(["hyprctl", "--instance", signature, *command], env=nested_env,
                                      capture_output=True, text=True, check=True, timeout=4).stdout
            def cli(*command):
                return subprocess.run([str(binary), *command], env=nested_env, capture_output=True,
                                      text=True, check=True, timeout=5).stdout.strip()
            def status():
                try:
                    return json.loads(cli("status"))
                except (subprocess.CalledProcessError, json.JSONDecodeError):
                    return None
            daemon_log = (base / "daemon.log").open("w+")
            daemon = subprocess.Popen([str(binary), "daemon"], env=nested_env, stdout=daemon_log, stderr=daemon_log)
            processes.append(daemon)
            wait_for(status, "daemon control socket")
            wait_for(lambda: (s := status()) and s.get("checker_ready"), "English engine ready", 15)
            # Parent PID and explicit isolated instance checked before the sole plugin load command.
            assert "ok" in ctl("plugin", "load", str(plugin)).lower()
            wait_for(lambda: (s := status()) and s["bridge_connected"], "bridge handshake")
            fixture, output = start_fixture()
            wait_for(lambda: (s := status()) and s["app"] == "org.example.WritingFixture" and s["capture"], "fixture capture")
            def focus(command):
                fixture.stdin.write((command + "\n").encode())
                fixture.stdin.flush()
                time.sleep(0.55)  # Accessibility inspection + one lease cycle.
            def type_fixture(text, during=None, accept_chord=False):
                # Creating a virtual keyboard can change keymaps: let the first reset settle,
                # then verify the actual client content independently of Writy's projection.
                arguments=["wtype", "-s", "600", "-d", "30", text, "-s", ("40000" if args.with_model else "6000") if during else "100"]
                if accept_chord:
                    arguments=arguments[:-2]+["-s","2000","-k","Print","-s","38000" if args.with_model else "4000"]
                if accept_chord: arguments.extend(["-M","ctrl","-M","alt","-k","Return","-m","alt","-m","ctrl","-s","250"])
                typing = subprocess.Popen(arguments, env=nested_env)
                processes.append(typing)
                if during is not None:
                    wait_for(lambda: output.exists() and output.read_text() == text, "typed fixture text")
                    during()
                assert typing.wait(timeout=50 if args.with_model else 12) == 0
                time.sleep(0.35)
            focus("clear")
            focus("prose")
            first = "I recieved the fixture."
            live = {}
            def check_live():
                wait_for(lambda: (s := status()) and s["buffer_chars"] > 0, "live projected segment")
                live.update(status())
                wait_for(lambda: (s := status()) and s["suggestions"] > 0, "Harper suggestion")
                wait_for(lambda: any(layer.get("namespace")=="writy" and layer.get("alpha",0)>0.9 for output in json.loads(ctl("-j", "layers")).values() for level in output["levels"].values() for layer in level), "painted writing HUD", 5)
                def writing_layers():
                    return [layer for output in json.loads(ctl("-j", "layers")).values() for level in output["levels"].values() for layer in level if layer.get("namespace")=="writy" and layer.get("alpha",0)>0.9]
                wait_for(lambda: len(writing_layers())==1, "unified reading HUD")
                shown=sorted(writing_layers(),key=lambda layer:layer["y"])
                monitor=json.loads(ctl("-j","monitors"))[0]
                logical_height=int(monitor["height"]/monitor["scale"])
                assert all(layer["y"]>=0 and layer["y"]+layer["h"]<=logical_height for layer in shown), shown
                assert all(a["y"]+a["h"]<=b["y"] for a,b in zip(shown,shown[1:])), shown
                # Moving onto a non-keyboard HUD must preserve the writing context.
                hover = shown[0]
                ctl("dispatch", "movecursor", f"{hover['x']+hover['w']//2} {hover['y']+15}")
                time.sleep(0.4)
                assert len(writing_layers()) == 1, "HUDs disappeared on hover"
                assert status().get("verified"), "Hover invalidated the text range"
                ctl("dispatch", "movecursor", "100 600")
                time.sleep(2.2)  # The existing virtual keyboard presses Print.
                assert len(writing_layers()) == 1, "Screenshot hotkey dismissed the HUDs"
                (base / "layers.json").write_text(ctl("-j", "layers"))
                if args.with_model:
                    wait_for(lambda: (state := status()) and state.get("dictionary_matches",0)>0, "generated insert-mode word details", 35)
                time.sleep(0.35)
                if args.screenshot:
                    args.screenshot.parent.mkdir(parents=True, exist_ok=True)
                    subprocess.run(["grim", str(args.screenshot.resolve())], env=nested_env, check=True, timeout=5)
                wait_for(lambda: (s := status()) and s.get("verified"), "verified text range")
            type_fixture(first, check_live, accept_chord=True)
            wait_for(lambda: output.exists() and output.read_text() == "I received the fixture.", "global acceptance shortcut and verified correction")
            focus("password")
            wait_for(lambda: (s := status()) and not s["capture"] and s["buffer_chars"] == 0, "secure field exclusion")
            type_fixture("syntheticsecret")
            assert output.read_text() == "I received the fixture."
            assert status()["buffer_chars"] == 0
            focus("prose")
            focus("clear")
            multilingual = "English العربية"
            type_fixture(multilingual)
            wait_for(lambda: output.read_text() == multilingual, "mixed-script input preserved")
            # Exercise the real importer/lookup/copy/search paths with all lexical categories.
            dictionary = base / "sample.jsonl"
            dictionary.write_text(json.dumps({"word":"decision","lang_code":"en","pos":"noun","senses":[{"glosses":["a choice made after considering alternatives"],"examples":[{"text":"She made a decision."}],"collocations":[{"word":"make a decision"}],"synonyms":[{"word":"choice"}],"antonyms":[{"word":"indecision"}],"derived":[{"word":"decisive"}]}],"translations":[{"lang_code":"ar","word":"قرار"}]})+"\n")
            cli("dict", "import", str(dictionary), "--language", "en", "--name", "Synthetic fixture", "--license", "CC0-1.0")
            lookup=json.loads(cli("lookup", "decision", "--json"))
            assert lookup[0]["senses"][0]["translations"]==["قرار"]
            cli("dismiss")
            focus("clear")
            focus("prose")
            type_fixture("decision")
            selecting=subprocess.Popen(["wtype","-s","600","-M","ctrl","-k","a","-m","ctrl","-s","40000"],env=nested_env)
            processes.append(selecting)
            wait_for(lambda: (state := status()) and state.get("selection_active") and state.get("dictionary_matches",0)>0,"automatic PRIMARY lookup")
            selected=json.loads(cli("lookup","--selection","--json"))
            assert selected[0]["headword"]=="decision"
            # Keep this virtual keyboard alive through the read/screenshot checks.
            # Its removal correctly invalidates capture, so teardown belongs after
            # the interactions being verified, not between lookup and presentation.
            assert selecting.poll() is None
            def layers():
                return [layer for output in json.loads(ctl("-j","layers")).values() for level in output["levels"].values() for layer in level if layer.get("namespace")=="writy" and layer.get("alpha",0)>0.9]
            cli("dismiss")
            wait_for(lambda: not layers(), "previous selection HUD hidden")
            ctl("dispatch", "movecursor", "120 120")
            time.sleep(0.15)
            cli("lookup", "decision")
            wait_for(lambda: len(layers())==1, "painted lexical HUD stack")
            def reading_layout_valid():
                shown=sorted(layers(),key=lambda layer:layer["y"])
                monitor=json.loads(ctl("-j","monitors"))[0]
                height=int(monitor["height"]/monitor["scale"])
                return bool(shown) and all(layer["y"]>=0 and layer["y"]+layer["h"]<=height for layer in shown) and all(a["y"]+a["h"]<=b["y"] for a,b in zip(shown,shown[1:])) and shown[-1]["y"]+shown[-1]["h"]-shown[0]["y"]<=min(680,int(height*.72))
            try:
                wait_for(reading_layout_valid,"reading HUD viewport budget after configure")
            except AssertionError:
                print("Reading HUD geometry:", json.dumps(layers()), flush=True)
                raise
            try:
                wait_for(lambda: len(layers()) == 1 and abs(layers()[0]["x"] - 132) <= 2 and abs(layers()[0]["y"] - 132) <= 2,
                         "selection HUD beside cursor")
            except AssertionError:
                print("Cursor placement:", ctl("-j", "cursorpos"), "HUD:", json.dumps(layers()), flush=True)
                raise
            hud_count = len(layers())
            focus("screenshot")
            wait_for(lambda: status().get("app", "").lower() == "org.flameshot.flameshot", "screenshot focus")
            def retained_screenshot_huds():
                return [layer for output in json.loads(ctl("-j", "layers")).values()
                        for entries in output["levels"].values() for layer in entries
                        if layer.get("namespace") == "writy"]
            wait_for(lambda: len(retained_screenshot_huds()) == hud_count, "HUDs retained behind screenshot controls")
            assert not status()["capture"], "Screenshot annotations must not be captured"
            assert not status()["verified"], "Screenshot focus must invalidate correction anchors"
            raw_layers = json.loads(ctl("-j", "layers"))
            assert any(layer.get("namespace") == "writy" for output in raw_layers.values()
                       for layer in output["levels"].get("2", [])), "HUDs must sit below screenshot controls"
            focus("prose")
            # Restore the explicit lookup after returning from the capture tool.
            cli("lookup", "decision")
            wait_for(lambda: len(layers()) == 1, "restored reading HUDs")

            cli("copy")
            wait_for(lambda: subprocess.run(["wl-paste","--no-newline"],env=nested_env,capture_output=True,text=True).stdout=="قرار", "translation copied without labels")
            selecting.terminate()
            selecting.wait(timeout=5)
            cli("search")
            wait_for(lambda: "io.github.lodaq.writy" in ctl("-j","clients"), "focused search window")
            time.sleep(0.5)
            if args.screenshot:
                subprocess.run(["grim", str(args.screenshot.with_stem(args.screenshot.stem+"-search").resolve())],env=nested_env,check=True,timeout=5)
            cli("settings")
            time.sleep(0.25)
            cli("dismiss")
            ctl("dispatch", "focuswindow", "class:^(org.example.WritingFixture)$")
            # Stopping the service leaves the stream open; key delivery must remain normal.
            memory_kib=int(next(line.split()[1] for line in Path(f"/proc/{daemon.pid}/status").read_text().splitlines() if line.startswith("VmRSS:")))
            daemon.send_signal(signal.SIGSTOP)
            focus("clear")
            type_fixture("hangfixture")
            wait_for(lambda: output.read_text() == "hangfixture", "input during daemon hang")
            daemon.send_signal(signal.SIGCONT)
            daemon.terminate()
            daemon.wait(timeout=5)
            focus("clear")
            type_fixture("exitfixture")
            wait_for(lambda: output.read_text() == "exitfixture", "input after daemon termination")
            assert "ok" in ctl("plugin", "unload", str(plugin)).lower()
            assert compositor.poll() is None
            print(json.dumps({"nested_only": True, "hyprland": "0.56.2", "bridge": True, "live_buffer": True,
                              "harper": True, "verified_edit": True, "dictionary_lookup": True, "primary_selection": True, "acceptance_shortcut": True, "clipboard": True, "search": True, "hud_budget": True, "secure_field": True, "mixed_script": True,
                              "input_preserved_live_hang_exit": True, "unload": True,
                              "observed_buffer_chars": live["buffer_chars"], "daemon_rss_kib":memory_kib, "artifacts": str(base) if args.keep_temp else None}))
        except Exception:
            daemon_path = base / "daemon.log"
            if args.keep_temp:
                print("Nested diagnostics:", base)
                try:
                    (base / "active-window.json").write_text(ctl("-j","activewindow"))
                    (base / "monitors.json").write_text(ctl("-j","monitors"))
                    (base / "final-layers.json").write_text(ctl("-j","layers"))
                except Exception: pass
            if daemon_path.exists():
                # Logs may contain fixture content: don't include them in the default result.
                print("Desktop verification failed; inspect local daemon/compositor diagnostics during a --pdb run.")
            raise
        finally:
            for process in reversed(processes):
                if process.poll() is None:
                    try:
                        process.send_signal(signal.SIGCONT)
                        process.terminate()
                    except ProcessLookupError:
                        pass
            for process in reversed(processes):
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


if __name__ == "__main__":
    main()

"""Opt-in ScreenCaptureKit source -> real Tauri host/viewer pixels and PCM checks on macOS."""
import argparse
from contextlib import contextmanager
import importlib
import hashlib
import json
import os
from pathlib import Path
import plistlib
import socket
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
fixtures = importlib.import_module("test-native-audio-isolation")
ROOT = Path(__file__).resolve().parent.parent


@contextmanager
def process(command, log, env=None):
    with log.open("w") as stream:
        child = subprocess.Popen(command, cwd=ROOT, env=env, stdout=stream, stderr=stream)
        try:
            yield child
        finally:
            if child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait(timeout=5)


def report(path):
    try:
        return json.loads(path.read_text())
    except (FileNotFoundError, json.JSONDecodeError):
        return {}


def await_host(child, path):
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        data = report(path)
        for result in data.get("results", []):
            if result["result"] != "PASS":
                raise RuntimeError("native host check failed; inspect " + str(path))
            if result["name"] == "Native host ready":
                return
        if child.poll() is not None:
            raise RuntimeError("native host exited before capture; inspect " + str(path))
        time.sleep(0.1)
    raise RuntimeError("native host startup timed out; inspect " + str(path))


def binary_identity(binary):
    """Record executable bytes and bundle context without inferring permission."""
    binary = binary.resolve()
    identity = {"path": str(binary), "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                "bundle": None}
    for parent in binary.parents:
        plist = parent / "Contents/Info.plist"
        if parent.suffix == ".app" and plist.is_file():
            metadata = plistlib.loads(plist.read_bytes())
            identity["bundle"] = {"path": str(parent),
                                  "identifier": metadata.get("CFBundleIdentifier")}
            break
    return identity


def run_case(binary, probe, bundles, tone, signaling, output, audio, lifecycle=False,
             viewer_binary=None):
    directory = output / ("source-lifecycle" if lifecycle else "audio-on" if audio else "audio-off")
    directory.mkdir()
    stop = directory / "host.stop"
    fixture = fixtures.Fixture(bundles["Tone"], tone, directory, direct=False)
    alternate = None
    fixture_closed = False
    try:
        fixture.ready(probe)
        env = dict(os.environ, VIDEO_P2P_GUI_QA_SIGNALING=signaling,
                   VIDEO_P2P_GUI_QA_NATIVE_SOURCE=fixture.window,
                   VIDEO_P2P_GUI_QA_NATIVE_AUDIO=str(audio).lower(),
                   VIDEO_P2P_GUI_QA_STOP_FILE=str(stop))
        if lifecycle:
            alternate = fixtures.Fixture(bundles["Silent"], tone, directory, direct=False)
            alternate.ready(probe)
            env["VIDEO_P2P_GUI_QA_ALTERNATE_SOURCE"] = alternate.window
        else:
            env.pop("VIDEO_P2P_GUI_QA_ALTERNATE_SOURCE", None)
        host_env = dict(env, VIDEO_P2P_MVP_CONFIG_DIR=str(directory / "host-config"),
                        VIDEO_P2P_GUI_QA_REPORT=str(directory / "host.json"),
                        VIDEO_P2P_GUI_QA_SCENARIO="native-host")
        viewer_env = dict(env, VIDEO_P2P_MVP_CONFIG_DIR=str(directory / "viewer-config"),
                          VIDEO_P2P_GUI_QA_REPORT=str(directory / "viewer.json"),
                          VIDEO_P2P_GUI_QA_SCENARIO="native-viewer")
        with process([str(binary)], directory / "host.log", host_env) as host:
            processes = {"host": {"pid": host.pid, "binary": str(binary)}}
            process_record = directory / "processes.json"
            process_record.write_text(json.dumps(processes, indent=2) + "\n")
            try:
                await_host(host, directory / "host.json")
                with process([str(viewer_binary or binary)], directory / "viewer.log", viewer_env) as viewer:
                    processes["viewer"] = {"pid": viewer.pid, "binary": str(viewer_binary or binary)}
                    process_record.write_text(json.dumps(processes, indent=2) + "\n")
                    if lifecycle:
                        deadline = time.monotonic() + 60
                        while not (directory / "restored-viewer").exists():
                            if viewer.poll() is not None or host.poll() is not None:
                                raise RuntimeError("client exited before source closure; inspect GUI reports")
                            if time.monotonic() >= deadline:
                                raise RuntimeError("source lifecycle timed out before restored playback")
                            time.sleep(0.1)
                        fixture.close()
                        fixture_closed = True
                        (directory / "source-closed").touch()
                    viewer_code = viewer.wait(timeout=60)
            finally:
                stop.touch()
            host_code = host.wait(timeout=15)
        reports = {role: report(directory / (role + ".json")) for role in ("host", "viewer")}
        success = host_code == viewer_code == 0 and all(
            data.get("complete") and data.get("success") for data in reports.values())
        for role, data in reports.items():
            for check in data.get("results", []):
                print(check["result"], directory.name, role, check["name"], flush=True)
        return {"audio": audio, "source": fixture.window, "success": success,
                "host_exit": host_code, "viewer_exit": viewer_code}
    finally:
        try:
            if alternate is not None:
                alternate.close()
        finally:
            if not fixture_closed:
                fixture.close()


def arguments(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path,
                        default=ROOT / "apps/desktop/src-tauri/target/debug/desktop-app",
                        help="host executable; also used for the viewer unless --viewer-binary is set")
    parser.add_argument("--viewer-binary", type=Path,
                        help="separate viewer executable for controlled launch-context comparisons")
    parser.add_argument("--probe", type=Path, default=ROOT / "target/debug/examples/capture_probe")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-lifecycle", action="store_true",
                        help="switch between distinct sources, restore the original, then close it")
    args = parser.parse_args(argv)
    if sys.platform != "darwin":
        parser.error("requires macOS, a desktop session and Screen Recording permission")
    args.binary = args.binary.resolve()
    args.viewer_binary = (args.viewer_binary or args.binary).resolve()
    for label, binary in (("host", args.binary), ("viewer", args.viewer_binary)):
        if not binary.is_file() or not os.access(binary, os.X_OK):
            parser.error(f"{label} executable is missing or not executable: {binary}")
    return args


def main():
    args = arguments()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    results = []
    summary = {"success": False, "results": results, "binary": str(args.binary.resolve()),
               "os": subprocess.check_output(["sw_vers"], text=True)}
    try:
        summary["clients"] = {role: {**binary_identity(binary), "launch": "direct"}
                              for role, binary in (("host", args.binary),
                                                   ("viewer", args.viewer_binary))}
        summary["binary_sha256"] = summary["clients"]["host"]["sha256"]
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            port = reservation.getsockname()[1]
        signaling = "127.0.0.1:" + str(port)
        with process([str(ROOT / "target/debug/signaling-server"), signaling], output / "server.log") as server:
            deadline = time.monotonic() + 5
            while True:
                try:
                    with socket.create_connection(("127.0.0.1", port), timeout=0.2):
                        break
                except OSError:
                    if server.poll() is not None or time.monotonic() >= deadline:
                        raise RuntimeError("local signaling did not start")
                    time.sleep(0.05)
            with tempfile.TemporaryDirectory(prefix="video-p2p-native-desktop-") as scratch:
                bundles, tone = fixtures.build_fixtures(Path(scratch), output)
                for audio in ((True,) if args.source_lifecycle else (True, False)):
                    results.append(run_case(args.binary.resolve(), args.probe.resolve(), bundles,
                                            tone, signaling, output, audio, args.source_lifecycle,
                                            args.viewer_binary))
        summary["success"] = all(result["success"] for result in results)
    except Exception as error:
        summary["error"] = str(error)
        raise
    finally:
        (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
        print("Evidence:", output, flush=True)
    return int(not summary["success"])


if __name__ == "__main__":
    raise SystemExit(main())

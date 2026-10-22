"""Opt-in macOS desktop capture regression; saves diagnostics, never captured media."""
import argparse
import json
import math
from pathlib import Path
import re
import struct
import subprocess
import sys
import tempfile
import time
import wave


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe", type=Path, default=Path("target/debug/examples/capture_probe"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("requires a macOS desktop session and Screen Recording access")
    probe = args.probe.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    results = []
    with tempfile.TemporaryDirectory(prefix="video-p2p-native-capture-") as scratch:
        scratch = Path(scratch)
        fixture = scratch / "fixture"
        source = Path(__file__).resolve().with_name("native-capture-fixture.m")
        subprocess.run(["xcrun", "clang", "-fobjc-arc", "-fblocks", "-Wall", "-Wextra",
                        "-Werror", str(source), "-framework", "Cocoa", "-o", str(fixture)], check=True)
        tone = scratch / "tone.wav"
        with wave.open(str(tone), "wb") as wav:
            wav.setparams((2, 2, 48000, 0, "NONE", "not compressed"))
            wav.writeframes(b"".join(struct.pack("<hh", *([round(
                0.05 * 32767 * math.sin(i * 440 * 2 * math.pi / 48000))] * 2))
                for i in range(48000)))

        # A fresh fixture per case prevents its bounded lifetime from expiring mid-capture.
        # Silent cases run without our tone process; cross-application isolation is separate QA.
        cases = [
            ("window-required", "window", "--require-audio", True, 0),
            ("window-include", "window", "--include-audio", True, 0),
            ("window-opt-out", "window", None, True, 0),
            ("application-required", "application", "--require-audio", True, 0),
            ("display-opt-out", "display", None, True, 0),
            ("silent-required", "window", "--require-audio", False, 1),
            ("silent-diagnostic", "window", "--include-audio", False, 0),
        ]
        for name, kind, flag, audible, expected in cases:
            print("RUN", name, flush=True)
            result = {"name": name, "result": "FAIL", "expected_exit": expected}
            with (output / (name + "-fixture.log")).open("w") as log:
                child = subprocess.Popen([str(fixture), str(tone) if audible else "--silent"],
                                         stdout=log, stderr=log)
                try:
                    deadline = time.monotonic() + 10
                    while True:
                        text = (output / (name + "-fixture.log")).read_text()
                        match = re.search(r"window_id=(\d+) process_id=(\d+)", text)
                        if match:
                            break
                        if child.poll() is not None or time.monotonic() >= deadline:
                            raise RuntimeError("fixture did not become ready: " + text)
                        time.sleep(0.1)
                    # A Cocoa window number precedes WindowServer catalog visibility.
                    window_id = "macos-window-" + match[1]
                    deadline = time.monotonic() + 15
                    while True:
                        catalog = subprocess.run([str(probe), "--list"], capture_output=True,
                                                 text=True, timeout=20, check=True).stdout
                        if re.search(r"^" + re.escape(window_id) + r"\t", catalog, re.M):
                            break
                        if child.poll() is not None or time.monotonic() >= deadline:
                            raise RuntimeError("fixture window missing from native catalog")
                        time.sleep(0.2)
                    (output / (name + "-catalog.log")).write_text(catalog)
                    if kind == "window":
                        source_id = window_id
                    else:
                        pattern = (r"^(macos-application-" + match[2] + r"-\d+)\t"
                                   if kind == "application" else r"^(macos-display-\d+)\t")
                        found = re.search(pattern, catalog, re.M)
                        if not found:
                            raise RuntimeError("native source missing; check Screen Recording access")
                        source_id = found[1]
                    command = [str(probe), source_id] + ([flag] if flag else [])
                    result["command"] = command
                    run = subprocess.run(command, capture_output=True, text=True, timeout=25)
                    text = run.stdout + run.stderr
                    (output / (name + ".log")).write_text(text)
                    result["exit"] = run.returncode
                    passed = run.returncode == expected and bool(re.search(r"captured video=[1-9]", text))
                    if name == "silent-required":
                        passed = passed and "no audio signal above RMS" in text
                    if flag is None:
                        passed = passed and bool(re.search(r"audio=0 samples=0", text))
                    result["result"] = "PASS" if passed else "FAIL"
                except (OSError, RuntimeError, subprocess.SubprocessError) as error:
                    result["error"] = str(error)
                finally:
                    if child.poll() is None:
                        child.terminate()
                    try:
                        child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        child.kill()
                        child.wait()
            results.append(result)
            (output / "results.json").write_text(json.dumps(results, indent=2) + "\n")
            print(result["result"], name, flush=True)
    return int(any(result["result"] != "PASS" for result in results))


if __name__ == "__main__":
    raise SystemExit(main())

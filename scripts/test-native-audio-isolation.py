"""Opt-in concurrent macOS application audio isolation; saves diagnostics, not captured media."""
import argparse
from contextlib import ExitStack
import hashlib
import json
import math
from pathlib import Path
import plistlib
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import wave


def build_fixtures(scratch, output):
    source = Path(__file__).resolve().with_name("native-capture-fixture.m")
    executable = scratch / "fixture"
    subprocess.run(["xcrun", "clang", "-fobjc-arc", "-fblocks", "-Wall", "-Wextra",
                    "-Werror", str(source), "-framework", "Cocoa", "-o", str(executable)], check=True)
    bundles = {}
    identities = {}
    for name in ("Tone", "Silent"):
        bundle = scratch / (name + ".app")
        binary = bundle / "Contents/MacOS/fixture"
        binary.parent.mkdir(parents=True)
        identifier = "dev.videop2p.qa.audio-isolation." + name.lower()
        (bundle / "Contents/Info.plist").write_bytes(plistlib.dumps({
            "CFBundleIdentifier": identifier, "CFBundleExecutable": "fixture",
            "CFBundleName": "VideoP2P " + name, "CFBundlePackageType": "APPL",
        }))
        shutil.copy2(executable, binary)
        subprocess.run(["codesign", "--force", "--sign", "-", "--identifier", identifier,
                        str(bundle)], check=True, capture_output=True)
        signature = subprocess.run(["codesign", "-dvv", str(bundle)], check=True,
                                   capture_output=True, text=True)
        (output / (name + "-signature.log")).write_text(signature.stderr)
        identities[name] = {"bundle_id": identifier,
                            "sha256": hashlib.sha256(binary.read_bytes()).hexdigest()}
        bundles[name] = bundle
    (output / "identities.json").write_text(json.dumps(identities, indent=2) + "\n")
    tone = scratch / "tone.wav"
    with wave.open(str(tone), "wb") as wav:
        wav.setparams((2, 2, 48000, 0, "NONE", "not compressed"))
        wav.writeframes(b"".join(struct.pack("<hh", *([round(
            0.05 * 32767 * math.sin(i * 440 * 2 * math.pi / 48000))] * 2))
            for i in range(48000)))
    return bundles, tone


class Fixture:
    def __init__(self, bundle, tone, directory, direct):
        self.name = bundle.stem
        self.stop_file = directory / (self.name + ".stop")
        self.log_path = directory / (self.name + "-fixture.log")
        self.log = self.log_path.open("w")
        arguments = [str(tone) if self.name == "Tone" else "--silent",
                     "--stop-file", str(self.stop_file)]
        command = ([str(bundle / "Contents/MacOS/fixture")] if direct else
                   ["/usr/bin/open", "-n", "-W", "--stdout", str(self.log_path),
                    "--stderr", str(self.log_path), str(bundle), "--args"])
        try:
            self.process = subprocess.Popen(command + arguments, stdout=self.log, stderr=self.log)
        except OSError:
            self.log.close()
            raise

    def ready(self, probe):
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            text = self.log_path.read_text()
            match = re.search(r"window_id=(\d+) process_id=(\d+)", text)
            if self.process.poll() is not None:
                raise RuntimeError("fixture exited during startup: " + text)
            if match:
                self.window = "macos-window-" + match[1]
                self.pid = int(match[2])
                expected_id = "bundle_id=dev.videop2p.qa.audio-isolation." + self.name.lower()
                if expected_id not in text:
                    raise RuntimeError("fixture has unexpected bundle identity: " + text)
                catalog = subprocess.run([str(probe), "--list"], check=True,
                                         capture_output=True, text=True, timeout=15).stdout
                app = re.search(r"^(macos-application-" + match[2] + r"-\d+)\t", catalog, re.M)
                if app and re.search(r"^" + self.window + r"\t", catalog, re.M):
                    self.application = app[1]
                    return
            time.sleep(0.2)
        raise RuntimeError("fixture did not appear in the native catalog")

    def close(self):
        # The application observes its own stop file; never signal a PID found in the user's catalog.
        self.stop_file.touch()
        started = time.monotonic()
        try:
            # open -W waits for the actual app. The fixture also has a 120-second watchdog.
            while True:
                try:
                    status = self.process.wait(timeout=5)
                    break
                except subprocess.TimeoutExpired:
                    if time.monotonic() - started >= 130:
                        raise RuntimeError("fixture did not exit after its stop request")
            (self.log_path.with_name(self.name + "-cleanup.json")).write_text(json.dumps({
                "exit": status, "stop_wait_seconds": time.monotonic() - started,
            }, indent=2) + "\n")
            if status != 0:
                raise RuntimeError("fixture launcher exited with status " + str(status))
        finally:
            self.log.close()


def capture(probe, name, source, flag, silent, directory):
    command = [str(probe), source] + ([flag] if flag else [])
    run = subprocess.run(command, capture_output=True, text=True, timeout=25)
    text = run.stdout + run.stderr
    (directory / (name + ".log")).write_text(text)
    match = re.search(r"captured video=(\d+) audio=(\d+) samples=(\d+) peak=([\d.]+) rms=([\d.]+)", text)
    result = {"name": name, "command": command, "exit": run.returncode, "result": "FAIL"}
    if match:
        video, audio, samples = map(int, match.group(1, 2, 3))
        peak, rms = map(float, match.group(4, 5))
        result.update(video=video, audio=audio, samples=samples, peak=peak, rms=rms)
        valid = run.returncode == 0 and video > 0
        if flag is None:
            valid = valid and audio == samples == 0
        elif silent:
            # Require delivered silent PCM, not an absent audio stream or failed source.
            valid = valid and audio > 0 and samples > 0 and peak <= 0.000001 and rms <= 0.000001
        else:
            valid = valid and audio > 0 and samples > 0 and rms > 0.001
        result["result"] = "PASS" if valid else "FAIL"
    return result


def run_suite(probe, bundles, tone, output, direct):
    mode = "direct" if direct else "launch-services"
    directory = output / mode
    directory.mkdir()
    results = []
    with ExitStack() as stack:
        fixtures = {}
        for name, bundle in bundles.items():
            fixture = Fixture(bundle, tone, directory, direct)
            stack.callback(fixture.close)
            fixture.ready(probe)
            fixtures[name] = fixture
        active, silent = fixtures["Tone"], fixtures["Silent"]
        cases = [
            ("tone-window", active.window, "--require-audio", False),
            ("silent-window", silent.window, "--include-audio", True),
            ("silent-application", silent.application, "--include-audio", True),
            ("tone-application", active.application, "--require-audio", False),
            ("window-opt-out", active.window, None, False),
            ("application-opt-out", active.application, None, False),
            # Positive control after the silence cases proves the tone has not simply stopped.
            ("tone-still-playing", active.window, "--require-audio", False),
        ]
        for name, source, flag, expect_silence in cases:
            print("RUN", mode, name, flush=True)
            result = capture(probe, name, source, flag, expect_silence, directory)
            results.append(result)
            (directory / "results.json").write_text(json.dumps(results, indent=2) + "\n")
            print(result["result"], mode, name, flush=True)
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe", type=Path, default=Path("target/debug/examples/capture_probe"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--compare-direct-launch", action="store_true",
                        help="also reproduce the old launch attribution failure using the same binaries")
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("requires a macOS desktop session and Screen Recording access")
    probe = args.probe.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="video-p2p-audio-isolation-") as scratch:
        bundles, tone = build_fixtures(Path(scratch), output)
        if args.compare_direct_launch:
            before = run_suite(probe, bundles, tone, output, direct=True)
            expected_failures = {"silent-window", "silent-application"}
            reproduced = all(
                (item["result"] == "FAIL" and item["exit"] == 0 and item.get("video", 0) > 0
                 and item.get("rms", 0) > 0.001) if item["name"] in expected_failures
                else item["result"] == "PASS" for item in before)
            if not reproduced:
                raise RuntimeError("direct-launch control did not reproduce BUG-014; inspect saved results")
        after = run_suite(probe, bundles, tone, output, direct=False)
    return int(any(item["result"] != "PASS" for item in after))


if __name__ == "__main__":
    raise SystemExit(main())

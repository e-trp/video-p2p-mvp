"""Portable launch-selection and evidence regressions for the native desktop runner."""
from contextlib import contextmanager, redirect_stderr
import importlib
import io
import json
from pathlib import Path
import plistlib
import tempfile
import unittest
from unittest.mock import Mock, patch

runner = importlib.import_module("test-native-desktop-playback")


class NativeDesktopRunnerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.host = self.executable(self.root / "host/desktop-app")
        self.viewer = self.executable(self.root / "viewer/desktop-app")

    def executable(self, path):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"controlled executable bytes")
        path.chmod(0o755)
        return path

    def arguments(self, *extra):
        with patch.object(runner.sys, "platform", "darwin"):
            return runner.arguments(["--binary", str(self.host), "--output",
                                     str(self.root / "results"), *extra])

    def test_default_preserves_the_same_executable_for_both_roles(self):
        args = self.arguments()
        self.assertEqual((args.binary, args.viewer_binary), (self.host, self.host))

    def test_explicit_viewer_does_not_change_host(self):
        args = self.arguments("--viewer-binary", str(self.viewer))
        self.assertEqual((args.binary, args.viewer_binary), (self.host, self.viewer))

    def test_invalid_viewer_is_rejected_before_starting_processes(self):
        for path in (self.root / "missing", self.root / "viewer", self.viewer):
            if path == self.viewer:
                path.chmod(0o644)
            with self.subTest(path=path), redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as error:
                self.arguments("--viewer-binary", str(path))
            self.assertEqual(error.exception.code, 2)

    def test_identity_distinguishes_same_bytes_in_different_launch_contexts(self):
        bundle = self.root / "QA Client.app"
        bundled = self.executable(bundle / "Contents/MacOS/desktop-app")
        (bundle / "Contents/Info.plist").write_bytes(
            plistlib.dumps({"CFBundleIdentifier": "dev.videop2p.qa.test"}))
        standalone = runner.binary_identity(self.host)
        identified = runner.binary_identity(bundled)
        self.assertEqual(standalone["sha256"], identified["sha256"])
        self.assertNotEqual(standalone["path"], identified["path"])
        self.assertIsNone(standalone["bundle"])
        self.assertEqual(identified["bundle"], {
            "path": str(bundle), "identifier": "dev.videop2p.qa.test"})

    def run_case(self, viewer=None, startup_error=None):
        launched = []
        exited = []

        @contextmanager
        def process(command, log, env):
            launched.append((command, env["VIDEO_P2P_GUI_QA_SCENARIO"]))
            child = Mock(pid=100 + len(launched))
            child.wait.return_value = 0
            try:
                yield child
            finally:
                exited.append(child.pid)

        fixture = Mock(window="macos-window-42")
        with patch.object(runner.fixtures, "Fixture", return_value=fixture), \
                patch.object(runner, "process", process), \
                patch.object(runner, "await_host", side_effect=startup_error), \
                patch.object(runner, "report", return_value={"complete": True, "success": True}):
            try:
                result = runner.run_case(self.host, self.root / "probe", {"Tone": "fixture"},
                                         "tone", "127.0.0.1:7000", self.root, True,
                                         viewer_binary=viewer)
                self.assertTrue(result["success"])
            finally:
                fixture.close.assert_called_once()
                self.assertEqual(len(exited), len(launched))
        return launched

    def test_selected_binaries_are_launched_and_pids_are_retained(self):
        launched = self.run_case(self.viewer)
        self.assertEqual(launched, [([str(self.host)], "native-host"),
                                    ([str(self.viewer)], "native-viewer")])
        processes = json.loads((self.root / "audio-on/processes.json").read_text())
        self.assertEqual(processes, {"host": {"pid": 101, "binary": str(self.host)},
                                     "viewer": {"pid": 102, "binary": str(self.viewer)}})
        self.assertTrue((self.root / "audio-on/host.stop").exists())

    def test_default_run_still_launches_the_same_binary_twice(self):
        self.assertEqual([command for command, _ in self.run_case()],
                         [[str(self.host)], [str(self.host)]])

    def test_startup_failure_retains_host_identity_and_cleans_up(self):
        with self.assertRaisesRegex(RuntimeError, "controlled startup failure"):
            self.run_case(self.viewer, RuntimeError("controlled startup failure"))
        processes = json.loads((self.root / "audio-on/processes.json").read_text())
        self.assertEqual(processes, {"host": {"pid": 101, "binary": str(self.host)}})
        self.assertTrue((self.root / "audio-on/host.stop").exists())


if __name__ == "__main__":
    unittest.main()

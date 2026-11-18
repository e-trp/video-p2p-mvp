# BUG-016: Isolate Source Discovery from Capture Clients

2026-11-18, base `a5a7575`, untagged `0.1.0`, macOS 27.2 (26B123) arm64; Rust 1.100.0, Node 26.13.1. The tested implementation changes only the native metadata catalog and its Rust diagnostic labels. ScreenCaptureKit stream startup, stop/drop, timeout, ownership, encoding, signaling and viewer playback are unchanged.

## Controlled Reproduction and Repair

A minimal Objective-C executable links the production capture bridge. One process captures the controlled tone window, stops/restarts that source, switches to a silent window, and restores the original. A second process only calls `vpp_sck_catalog` every 200 ms. It starts after the first host capture receives samples and stays alive during restart. Both sources are the existing signed, LaunchServices-launched fixtures. Captured media is discarded after counting samples.

| Catalog implementation | Concurrent client | Result |
| --- | --- | --- |
| Original SCShareableContent | None | Four phases pass with video/audio |
| Original SCShareableContent | Same standalone executable path | Initial capture passes; first restart has zero samples and `ScreenCaptureKit startup timed out`; catalog queries also time out |
| Original SCShareableContent | Byte-identical executable at a different path | Four phases pass |
| CoreGraphics metadata | None, same path, different path | All three runs pass all four phases with video/audio |

Evidence is retained in `/private/tmp/video-p2p-bug016-nov18/` and `/private/tmp/video-p2p-bug016-nov18-after/`: `minimal.m`, `run.py`/`run-after.py`, binaries, phase logs, fixture identities/cleanup and results. The [normalized results](BUG-016-catalog-isolation-results.json) preserve the observed outcomes in the repository. The maintained end-to-end regression is `scripts/test-native-desktop-playback.py --source-lifecycle`; the temporary minimal harness is investigation evidence.

This reduces the failure below Tauri, Rust worker threads, encoding and WebRTC. Concurrent catalog-only ScreenCaptureKit use is sufficient to trigger it. The exact internal macOS client identity mechanism is not established. Different executable paths are controls, not the repair.

Catalog discovery now uses `CGWindowListCopyWindowInfo` and `CGGetActiveDisplayList`, so viewing source metadata does not initialize a ScreenCaptureKit connection. It retains `CGPreflightScreenCaptureAccess`, the existing native source IDs, audio capability and label fallbacks. Application entries are deduplicated by PID per intersecting display. Malformed IDs/bounds and nonzero-layer windows are excluded. Failed native queries still return errors for the existing cached-catalog handling. Capture startup resolves the selected ID against fresh SCShareableContent and can reject a source that vanished or is unavailable there.

CoreGraphics metadata enumeration is a documented public API; it does not capture images. See [Apple window metadata documentation](https://developer.apple.com/documentation/coregraphics/cgwindowlistcopywindowinfo(_:_:)) and [active display enumeration](https://developer.apple.com/documentation/coregraphics/cggetactivedisplaylist(_:_:_:)). No dependency, public Rust API, preference format, permission grant or installed application is changed.

## Actual Desktop Acceptance

```bash
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --features gui-qa
cargo build -p signaling-server
cargo build -p capture-macos --example capture_probe
python3 scripts/test-native-desktop-playback.py --source-lifecycle --output /private/tmp/video-p2p-bug016-nov18-desktop-after
python3 -B scripts/test-native-desktop-playback.py --source-lifecycle --output /private/tmp/video-p2p-bug016-nov18-desktop-repeat
python3 -B scripts/test-native-capture.py --output /private/tmp/video-p2p-bug016-nov18-source-types
```

Both desktop runs pass **17/17**, using two direct-launched clients at exactly the same standalone path/hash with permission granted. They verify real red/tone capture, blue/silent replacement, restoration of original pixels/nonzero PCM without replacing the viewer peer/generation, source closure, mute controls and Stop cleanup. The additional native suite passes **7/7**: window audio required/include/opt-out, application audio, display opt-out, expected silent-source rejection and permitted silent-source capture. These runs use one physical display; synthetic catalog tests additionally cover intersecting displays with negative coordinates.

The initial sandboxed `capture_probe --list` reported permission required and fallback sources. The same passive command outside the sandbox reported granted/runtime. Actual capture checks ran outside the command sandbox; no TCC settings were changed. This distinction is retained rather than treating a permission context difference as a product defect.

## Automated Verification and Limits

Native extraction tests now also cover source IDs/labels, per-display PID deduplication, missing or malformed metadata, invalid geometry, empty catalogs and audio availability. Existing buffer extraction, queue bounds and stop checks remain in the same permission-free suite.

All 19 commands in `/private/tmp/video-p2p-bug016-nov18/checks/results.json` exit 0; numbered logs preserve their output. The normalized results also contain each exact command and exit status:

- Format and format checks in both workspaces.
- Strict all-target/all-feature Clippy in both workspaces.
- All-feature Rust tests/doctests in both workspaces and playback-probe example tests.
- Viewer JavaScript lifecycle, probe-cleanup and seven portable native-runner tests.
- Permission-free native catalog/extraction/lifecycle tests.
- Signaling/playback-probe builds and WKWebView/Chrome playback and keyframe recovery (four modes).

`node scripts/test-tauri-gui.mjs --scenario source-picker --output /private/tmp/video-p2p-bug016-nov18-picker` also passes **8/8**, covering focused refresh, delayed IPC, audio opt-out, stale-source rejection and failure-state retention. The initial native test compile found a macro-parenthesis issue in the new assertions; it was corrected before the passing native suite and mandatory gate. No failing assertion or product failure is omitted from acceptance.

README, architecture, user/capture/GUI guides, TODO and the checklist were reviewed and updated for metadata discovery and verified standalone restart. Validation passes for 220 relative links/anchors and 23 shell blocks in the affected guides, JSON evidence, tracked-text Cyrillic scanning (zero matches), and `git diff --check`. Historical investigation records retain their original results.

After all verification finished, direct process inspection found no remaining QA/build processes. An initial process-list attempt through a sandboxed Python subprocess was denied; the allowed direct `ps -axo pid,ppid,command` check succeeded. `CARGO_TARGET_DIR` is unset and Cargo metadata identifies both repository-local targets. `cargo clean` exits 0, removing 11,438 files / 2.2 GiB; the separate desktop clean exits 0, removing 11,406 files / 2.8 GiB. Evidence and minimal reproduction artifacts remain outside target directories. Generated Python bytecode was removed. Full-project release QA remains open: physical audio devices, broader permission transitions, multiple physical displays, cross-network and distribution acceptance are not established by this fix. BUG-023's LaunchServices negotiation failure remains separate. No feature count, version or release tag is advanced.

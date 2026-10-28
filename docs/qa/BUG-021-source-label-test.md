# BUG-021: Compare Source Labels from the Applied Catalog

2026-10-28, untagged `0.1.0`, base `76375d6` plus BUG-019 QA-driver changes, macOS 27.0 (26A428), arm64. Ownership: `BASE-DESKTOP-CONTROLS`. Production session/capture behavior is unchanged.

The first BUG-019 mandatory run fails `cargo test --all --all-features` in `selecting_capture_source_updates_source_label`: 65 app-core tests pass, one fails. `/private/tmp/video-p2p-bug019/checks/7.log` retains the assertion and the two externally supplied Chrome window titles. The selected ID and audio assertions pass. This is a stale expected-label failure, not evidence of a permission or capture defect.

`select_source_with_retry` first enumerates real sources, then `select_capture_source` refreshes the catalog. A window title can change between those operations. The test used the first title as its expected value even though the selected snapshot correctly used the refreshed label. No permission denial or unavailable-source error occurs in this failure.

The assertion now uses the matching source in the catalog already applied by selection, with no additional OS query. It still checks the requested source ID and audio value. A controlled in-memory rename then requires `QA application - Renamed QA window` while preserving that ID/audio choice. This verifies metadata updates independently of real window timing and changes no actual user window. Existing failed-refresh/recovery/removal tests remain intact.

## Verification

`cargo test -p app-core selecting_capture_source_updates_source_label --all-features` passes, including the controlled rename and unchanged identity/audio assertions (`/private/tmp/video-p2p-bug019/label-fixed.log`). The corrected full root run passes all 127 tests and doctests; the original failure is preserved separately.

The final gate is shared with the QA-only BUG-019 changes. `/private/tmp/video-p2p-bug019/checks-final/results.json` records every exact command/exit code; logs 1–17 all pass: both workspace format/strict Clippy/test commands, four playback-probe unit tests, eight Node lifecycle tests, native extraction, supporting builds and ordinary/recovery playback in both WKWebView and Chrome. The GUI QA build and focused visibility run pass (8/8); the full GUI run records 43 PASS / 0 FAIL / 1 BLOCKED for the independent denied-permission scenario. This blocked permission scenario is not a source-label regression.

After all runners exited, process inspection found no remaining test/build processes. Both targets were repository-local and `CARGO_TARGET_DIR` was unset. `cargo clean` removed 9,603 files / 2.0 GiB; the separate desktop clean removed 11,143 files / 3.3 GiB. Both exited 0 and both target directories are absent. Logs remain outside those directories. JavaScript syntax, tracked-text language scan, Markdown links/anchors, shell-block syntax and `git diff --check` pass. Later documentation recording does not change the tested implementation.

README, TODO, architecture, GUI/viewer guides, QA checklist and bug entries were reviewed for the combined verification work. No production source-selection contract changed. No feature ID, version bump, release tag or installed-application acceptance is claimed. Fixing commit: `e4a1ad6`. BUG-021 is CLOSED; this test correction is committed separately from BUG-019.

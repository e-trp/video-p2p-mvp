#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
capture_test_dir=$(mktemp -d "${TMPDIR:-/tmp}/video-p2p-capture-test.XXXXXX")
trap 'rm -rf "$capture_test_dir"' EXIT HUP INT TERM
capture_sdk=$(xcrun --show-sdk-path)
xcrun clang -fobjc-arc -fblocks -Wall -Wextra -Werror -mmacosx-version-min=12.3 \
    -isysroot "$capture_sdk" "$project_dir/crates/capture-macos/native/screen_capture_test.m" \
    -framework Foundation -framework ScreenCaptureKit -framework CoreMedia \
    -framework CoreVideo -framework CoreGraphics -o "$capture_test_dir/capture-test"
"$capture_test_dir/capture-test"

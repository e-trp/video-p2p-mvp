use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=native/screen_capture.m");
    println!("cargo:rerun-if-changed=native/screen_capture.h");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let sdk = Command::new("xcrun")
        .arg("--show-sdk-path")
        .output()
        .expect("macOS capture requires Xcode Command Line Tools");
    assert!(sdk.status.success(), "xcrun could not locate the macOS SDK");
    let sdk = String::from_utf8(sdk.stdout).expect("SDK path is UTF-8");
    let arch = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86_64") => "x86_64",
        other => panic!("unsupported macOS capture architecture: {other:?}"),
    };
    let status = Command::new("xcrun")
        .args([
            "clang",
            "-c",
            "-fobjc-arc",
            "-fblocks",
            "-Wall",
            "-Wextra",
            "-Werror",
        ])
        .args([
            "-isysroot",
            sdk.trim(),
            "-arch",
            arch,
            "-mmacosx-version-min=12.3",
        ])
        .arg("native/screen_capture.m")
        .arg("-o")
        .arg(out.join("screen_capture.o"))
        .status()
        .expect("compile ScreenCaptureKit bridge");
    assert!(
        status.success(),
        "ScreenCaptureKit bridge compilation failed"
    );
    let status = Command::new("xcrun")
        .args(["ar", "crs"])
        .arg(out.join("libvideo_p2p_capture.a"))
        .arg(out.join("screen_capture.o"))
        .status()
        .expect("archive ScreenCaptureKit bridge");
    assert!(status.success(), "ScreenCaptureKit bridge archive failed");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=video_p2p_capture");
    for framework in [
        "Foundation",
        "ScreenCaptureKit",
        "CoreMedia",
        "CoreVideo",
        "CoreGraphics",
    ] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
    println!("cargo:rustc-link-lib=objc");
}

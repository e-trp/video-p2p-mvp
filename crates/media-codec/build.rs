use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=native/h264.m");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let sdk = Command::new("xcrun")
        .arg("--show-sdk-path")
        .output()
        .expect("macOS encoding requires Xcode Command Line Tools");
    assert!(sdk.status.success(), "xcrun could not locate the macOS SDK");
    let sdk = String::from_utf8(sdk.stdout).expect("SDK path is UTF-8");
    let arch = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86_64") => "x86_64",
        other => panic!("unsupported macOS encoding architecture: {other:?}"),
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
        .arg("native/h264.m")
        .arg("-o")
        .arg(out.join("h264.o"))
        .status()
        .expect("compile VideoToolbox codec bridge");
    assert!(
        status.success(),
        "VideoToolbox codec bridge compilation failed"
    );
    let status = Command::new("xcrun")
        .args(["ar", "crs"])
        .arg(out.join("libvideo_p2p_codec.a"))
        .arg(out.join("h264.o"))
        .status()
        .expect("archive VideoToolbox codec bridge");
    assert!(status.success(), "VideoToolbox codec bridge archive failed");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=video_p2p_codec");
    for framework in [
        "Foundation",
        "VideoToolbox",
        "CoreMedia",
        "CoreVideo",
        "CoreGraphics",
    ] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
    println!("cargo:rustc-link-lib=objc");
}

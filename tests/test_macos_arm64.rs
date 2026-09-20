use std::{path::Path, process::Command};

#[test]
fn macos_arm64_validation_script_is_available() {
    assert!(cfg!(all(target_os = "macos", target_arch = "aarch64")), "this validation target must run on macOS ARM64");
    assert!(Path::new("scripts/validate-macos-arm64.sh").is_file());
}

#[test]
fn macos_arm64_validation_script_runs_the_supported_flow_suites() {
    let output = Command::new("sh")
        .arg("scripts/validate-macos-arm64.sh")
        .output()
        .expect("macOS ARM64 validation script should start");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}

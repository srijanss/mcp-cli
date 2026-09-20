use std::{fs, process::Command};

#[test]
fn release_workflow_generates_and_publishes_sha256_checksums() {
    let workflow = fs::read_to_string(".github/workflows/release.yml")
        .expect("release workflow should exist");

    assert!(workflow.contains("sha256sum"));
    assert!(workflow.contains("SHA256SUMS"));
    assert!(workflow.contains("sha256sum -c SHA256SUMS"));
    assert!(workflow.contains("release/SHA256SUMS"));
}

#[test]
fn checksum_manifest_verifies_against_the_downloaded_release_asset_names() {
    let workflow = fs::read_to_string(".github/workflows/release.yml")
        .expect("release workflow should exist");

    assert!(workflow.contains("mkdir release"));
    assert!(workflow.contains("sha256sum mcpctl-* > SHA256SUMS"));
    assert!(workflow.contains("sha256sum -c SHA256SUMS"));
    assert!(workflow.contains("release/mcpctl-*"));
}

#[test]
fn publish_stage_stages_release_binaries_before_calculating_checksums() {
    let workflow = fs::read_to_string(".github/workflows/release.yml")
        .expect("release workflow should exist");

    assert!(workflow.contains("find artifacts -type f -name 'mcpctl-*' -exec cp {} release/ \\;"));
    assert!(workflow.contains("cd release"));
}

#[test]
fn checksum_staging_helper_creates_a_verifiable_manifest_for_release_assets() {
    let root = std::env::temp_dir().join(format!("mcpctl-release-checksum-{}", std::process::id()));
    let artifacts = root.join("artifacts/target-a");
    fs::create_dir_all(&artifacts).unwrap();
    fs::write(artifacts.join("mcpctl-target-a"), "binary contents").unwrap();

    let output = Command::new("sh")
        .args(["scripts/stage-release-checksums.sh"])
        .arg(root.join("artifacts"))
        .arg(root.join("release"))
        .output()
        .expect("checksum staging helper should start");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let manifest = fs::read_to_string(root.join("release/SHA256SUMS")).unwrap();
    assert!(manifest.contains("mcpctl-target-a"));
    assert!(!manifest.contains("artifacts/"));
    assert!(Command::new("sh")
        .arg("-c")
        .arg("cd \"$1\" && sha256sum -c SHA256SUMS")
        .arg("sh")
        .arg(root.join("release"))
        .status()
        .unwrap()
        .success());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn checksum_staging_helper_script_is_available() {
    assert!(std::path::Path::new("scripts/stage-release-checksums.sh").is_file());
}

#[test]
fn release_workflow_invokes_the_tested_checksum_staging_helper() {
    let workflow = fs::read_to_string(".github/workflows/release.yml").unwrap();
    assert!(workflow.contains("scripts/stage-release-checksums.sh artifacts release"));
}

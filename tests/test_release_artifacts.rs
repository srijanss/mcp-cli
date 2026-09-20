use std::fs;

#[test]
fn release_workflow_builds_stable_artifacts_for_supported_platforms() {
    let workflow = fs::read_to_string(".github/workflows/release.yml")
        .expect("release workflow should exist");

    for target in [
        "aarch64-apple-darwin",
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
    ] {
        assert!(
            workflow.contains(target),
            "release workflow should build {target}"
        );
    }

    assert!(workflow.contains("mcpctl-"), "artifacts should use the stable mcpctl name");
}

#[test]
fn release_workflow_publishes_binaries_from_a_tagged_release() {
    let workflow = fs::read_to_string(".github/workflows/release.yml")
        .expect("release workflow should exist");

    assert!(workflow.contains("tags:"), "workflow should run for release tags");
    assert!(workflow.contains("actions/checkout"));
    assert!(workflow.contains("softprops/action-gh-release"));
}

#[test]
fn arm64_linux_release_build_configures_an_aarch64_cross_linker() {
    let workflow = fs::read_to_string(".github/workflows/release.yml")
        .expect("release workflow should exist");

    assert!(workflow.contains("gcc-aarch64-linux-gnu"));
    assert!(workflow.contains("CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER"));
    assert!(workflow.contains("aarch64-linux-gnu-gcc"));
}

#[test]
fn arm64_linux_matrix_job_installs_the_configured_cross_linker() {
    let workflow = fs::read_to_string(".github/workflows/release.yml")
        .expect("release workflow should exist");

    assert!(workflow.contains("sudo apt-get install -y gcc-aarch64-linux-gnu"));
    assert!(workflow.contains("aarch64-linux-gnu-gcc"));
}

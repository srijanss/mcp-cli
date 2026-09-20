use std::{path::Path, process::Command};

#[test]
fn dockerfile_exposes_isolation_proof_stage() {
    let dockerfile = std::fs::read_to_string("Dockerfile")
        .expect("Dockerfile is required for the Docker isolation proof");
    assert!(dockerfile.contains("AS isolation-proof"));
}

#[test]
fn docker_isolation_proof_is_not_silently_skipped() {
    let source = std::fs::read_to_string("tests/test_docker.rs").unwrap();
    assert!(!source.contains(&["skipping", "Docker isolation proof"].join(" ")));
}

#[test]
#[ignore = "requires a Docker image build; run with --ignored"]
fn docker_sandbox_installs_and_runs_incompatible_python_mcp_dependencies_in_isolated_runtimes() {
    assert!(Path::new("Dockerfile").is_file(), "Dockerfile is required for the Docker isolation proof");

    let docker_available = Command::new("docker")
        .args(["info", "--format", "{{.ServerVersion}}"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    assert!(docker_available, "Docker is required for the isolation proof");

    let image = format!("mcpctl-isolation-test-{}", std::process::id());
    let build = Command::new("docker")
        .args(["build", "--target", "isolation-proof", "--tag", &image, "."])
        .output()
        .expect("Docker build should start");
    assert!(
        build.status.success(),
        "Docker isolation image failed to build:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new("docker")
        .args(["run", "--rm", &image])
        .output()
        .expect("Docker isolation proof should start");
    assert!(run.status.success(), "Docker isolation proof failed:\n{}", String::from_utf8_lossy(&run.stderr));
    assert_eq!(run.stdout, b"legacy:1.26.18\nmodern:2.2.3\n");
}

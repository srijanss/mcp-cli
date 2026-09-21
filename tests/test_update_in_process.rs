use std::{fs, path::PathBuf, process::{Command, Output, Stdio}, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_project() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!("mcpctl-update-test-{}-{}", std::process::id(), NEXT_ID.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(&path).unwrap();
    write_python_manifest(&path, "1.0.0");
    fs::write(path.join("pyproject.toml"), "[project]\nname = \"example-mcp\"\nversion = \"1.0.0\"\n").unwrap();
    fs::write(path.join("uv.lock"), "version = 1\n").unwrap();
    path
}

fn write_python_manifest(project: &PathBuf, version: &str) {
    fs::write(
        project.join("mcpctl.toml"),
        format!("name = \"example-mcp\"\nversion = \"{version}\"\n[runtime]\ntype = \"python\"\npython = \">=3.12\"\n[install]\nstrategy = \"uv\"\nentrypoint = \"example-mcp\"\n"),
    )
    .unwrap();
}

/// Writes a fake `uv` that records its parent pid (the `mcp-cli` process that ran it) into
/// `<project>/uv-parent-pid`, then exits with `exit_code`.
#[cfg(unix)]
fn fake_uv(project: &PathBuf, exit_code: i32) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let bin = project.join("fake-bin");
    fs::create_dir_all(&bin).unwrap();
    let script = format!("#!/bin/sh\necho $PPID > \"{}\"\nexit {exit_code}\n", project.join("uv-parent-pid").display());
    fs::write(bin.join("uv"), script).unwrap();
    fs::set_permissions(bin.join("uv"), fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

fn mcpctl(state_home: &PathBuf, bin: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(args).env("MCPCTL_HOME", state_home).env("PATH", bin).output().unwrap()
}

fn active_version(state_home: &PathBuf) -> String {
    let active: serde_json::Value = serde_json::from_str(&fs::read_to_string(state_home.join("active/example-mcp.json")).unwrap()).unwrap();
    active["version"].as_str().unwrap().to_owned()
}

#[cfg(unix)]
#[test]
fn update_runs_the_install_in_process() {
    let project = temporary_project();
    let bin = fake_uv(&project, 0);
    let state_home = project.join("state");
    assert!(mcpctl(&state_home, &bin, &["install", project.to_str().unwrap()]).status.success());
    write_python_manifest(&project, "2.0.0");

    let child = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["update", "example-mcp", "--source", project.to_str().unwrap()])
        .env("MCPCTL_HOME", &state_home)
        .env("PATH", &bin)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let update_pid = child.id();
    assert!(child.wait_with_output().unwrap().status.success());

    let uv_parent: u32 = fs::read_to_string(project.join("uv-parent-pid")).unwrap().trim().parse().unwrap();
    assert_eq!(uv_parent, update_pid, "install ran in a separate mcp-cli process instead of in-process");
    assert_eq!(active_version(&state_home), "2.0.0");

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn failed_update_leaves_the_previous_version_active() {
    let project = temporary_project();
    let state_home = project.join("state");
    let good_bin = fake_uv(&project, 0);
    assert!(mcpctl(&state_home, &good_bin, &["install", project.to_str().unwrap()]).status.success());
    write_python_manifest(&project, "2.0.0");
    let failing_bin = fake_uv(&project, 1);

    let output = mcpctl(&state_home, &failing_bin, &["update", "example-mcp", "--source", project.to_str().unwrap()]);

    assert!(!output.status.success());
    assert_eq!(active_version(&state_home), "1.0.0");
    assert!(!state_home.join("packages/example-mcp/2.0.0").exists());
    let registry = fs::read_to_string(state_home.join("registry.json")).unwrap();
    assert!(!registry.contains("2.0.0"), "{registry}");

    fs::remove_dir_all(project).unwrap();
}

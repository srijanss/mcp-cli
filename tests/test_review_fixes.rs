use std::{fs, path::PathBuf, process::{Command, Output}, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_project() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-review-fixes-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
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

/// Installs the fixture with a no-op fake `uv`; returns the process output and the state home.
#[cfg(unix)]
fn install(project: &PathBuf) -> (Output, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let bin = project.join("fake-bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("uv"), "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(bin.join("uv"), fs::Permissions::from_mode(0o755)).unwrap();
    let state_home = project.join("state");
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["install", project.to_str().unwrap()])
        .env("MCPCTL_HOME", &state_home)
        .env("PATH", &bin)
        .output()
        .unwrap();
    (output, state_home)
}

fn mcpctl(state_home: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(args).env("MCPCTL_HOME", state_home).output().unwrap()
}

fn read_json(path: PathBuf) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(&path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))).unwrap()
}

#[cfg(unix)]
#[test]
fn failed_active_write_rolls_back_install() {
    let project = temporary_project();
    let state_home = project.join("state");
    fs::create_dir_all(&state_home).unwrap();
    // A regular file where the `active/` directory belongs makes the active-selection write fail.
    fs::write(state_home.join("active"), "blocker").unwrap();

    let (output, _) = install(&project);

    assert!(!output.status.success());
    assert!(!state_home.join("packages/example-mcp/1.0.0").exists(), "failed install left its files behind");
    let registry = fs::read_to_string(state_home.join("registry.json")).unwrap_or_default();
    assert!(!registry.contains("example-mcp"), "failed install left a registry entry: {registry}");

    // With the blocker gone, a retry must not be rejected as "already installed".
    fs::remove_file(state_home.join("active")).unwrap();
    let (retry, _) = install(&project);
    assert!(retry.status.success(), "stderr was: {}", String::from_utf8_lossy(&retry.stderr));

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn failed_active_write_leaves_use_unchanged() {
    let project = temporary_project();
    let (first, state_home) = install(&project);
    assert!(first.status.success());
    write_python_manifest(&project, "2.0.0");
    assert!(install(&project).0.status.success());
    // Replace the `active/` directory with a file so writing the active selection fails.
    fs::remove_dir_all(state_home.join("active")).unwrap();
    fs::write(state_home.join("active"), "blocker").unwrap();

    let output = mcpctl(&state_home, &["use", "example-mcp@2.0.0"]);

    assert!(!output.status.success());
    let registry = read_json(state_home.join("registry.json"));
    assert_eq!(registry["packages"]["example-mcp"]["active_version"], "1.0.0", "registry moved although the active file could not be written");

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn python_install_without_uv_lock_omits_the_lockfile_hash() {
    let project = temporary_project();
    fs::remove_file(project.join("uv.lock")).unwrap();

    let (output, state_home) = install(&project);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    let metadata = read_json(state_home.join("packages/example-mcp/1.0.0/metadata.json"));
    assert!(metadata.get("lockfile_sha256").is_none(), "unexpected lockfile hash: {metadata}");
    assert!(metadata["manifest_sha256"].is_string());

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn missing_binary_entrypoint_error_does_not_mention_python() {
    let project = temporary_project();
    fs::write(project.join("mcpctl.toml"), "name = \"bin-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"missing-bin\"\n").unwrap();
    let state_home = project.join("state");

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["install", project.to_str().unwrap()])
        .env("MCPCTL_HOME", &state_home)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing-bin"), "{stderr}");
    assert!(!stderr.contains("Python"), "binary error should not mention Python: {stderr}");

    fs::remove_dir_all(project).unwrap();
}

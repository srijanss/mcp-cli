use std::{fs, path::PathBuf, process::{Command, Output}, sync::atomic::{AtomicUsize, Ordering}};

const MANIFEST: &str = r#"name = "scaf-mcp"
version = "1.0.0"

[runtime]
type = "binary"

[install]
entrypoint = "scaf-mcp"

[scaffold]
dirs = [".agents"]
exclude = [".agents/private.txt", "templates/mcp.json"]

[[scaffold.files]]
from = "templates/mcp.json"
to = ".mcp.json"

[[scaffold.hints]]
message = "Defaults to pytest."

[[scaffold.hints]]
when_exists = "Cargo.toml"
message = "Rust project detected: use cargo-adapter-runner."

[[scaffold.hints]]
when_exists = "package.json"
message = "JS/TS project detected: use vitest-adapter-runner."
"#;

fn temporary_dir(label: &str) -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!("mcpctl-init-{label}-{}-{}", std::process::id(), NEXT_ID.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(&path).unwrap();
    path
}

/// A binary MCP project that ships a scaffold, installed into a fresh `MCPCTL_HOME`.
#[cfg(unix)]
fn installed_scaffold_mcp() -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let project = temporary_dir("project");
    fs::write(project.join("mcpctl.toml"), MANIFEST).unwrap();
    fs::write(project.join("scaf-mcp"), "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(project.join("scaf-mcp"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::create_dir_all(project.join("templates")).unwrap();
    fs::write(project.join("templates/mcp.json"), "{\"from\":\"template\"}\n").unwrap();
    fs::create_dir_all(project.join(".agents/skills")).unwrap();
    fs::write(project.join(".agents/skills/tdd.md"), "skill\n").unwrap();
    fs::write(project.join(".agents/private.txt"), "secret\n").unwrap();
    let state_home = project.join("state");
    let installed = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["install", project.to_str().unwrap()]).env("MCPCTL_HOME", &state_home).output().unwrap();
    assert!(installed.status.success(), "install failed: {}", String::from_utf8_lossy(&installed.stderr));
    (project, state_home)
}

fn init(state_home: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mcp-cli")).arg("init").args(args).env("MCPCTL_HOME", state_home).output().unwrap()
}

#[cfg(unix)]
#[test]
fn init_copies_scaffold_files_and_dirs_from_the_installed_package_into_the_target() {
    let (project, state_home) = installed_scaffold_mcp();
    let target = temporary_dir("target");

    let output = init(&state_home, &["scaf-mcp", target.to_str().unwrap()]);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(fs::read_to_string(target.join(".mcp.json")).unwrap(), "{\"from\":\"template\"}\n");
    assert_eq!(fs::read_to_string(target.join(".agents/skills/tdd.md")).unwrap(), "skill\n");
    assert!(!target.join(".agents/private.txt").exists(), "excluded files must not be copied");
    assert!(!target.join("templates").exists(), "only declared scaffold entries are copied");

    fs::remove_dir_all(project).unwrap();
    fs::remove_dir_all(target).unwrap();
}

#[cfg(unix)]
#[test]
fn init_never_overwrites_existing_files_and_reports_what_it_skipped_and_copied() {
    let (project, state_home) = installed_scaffold_mcp();
    let target = temporary_dir("target");
    fs::write(target.join(".mcp.json"), "mine\n").unwrap();

    let output = init(&state_home, &["scaf-mcp", target.to_str().unwrap()]);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(fs::read_to_string(target.join(".mcp.json")).unwrap(), "mine\n", "existing file must be kept");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Skipping .mcp.json (already exists)"), "stdout was: {stdout}");
    assert!(stdout.contains("Copied .agents/skills/tdd.md"), "stdout was: {stdout}");
    assert_eq!(fs::read_to_string(target.join(".agents/skills/tdd.md")).unwrap(), "skill\n");

    fs::remove_dir_all(project).unwrap();
    fs::remove_dir_all(target).unwrap();
}

#[cfg(unix)]
#[test]
fn init_prints_only_the_hints_whose_marker_file_exists_in_the_target() {
    let (project, state_home) = installed_scaffold_mcp();
    let target = temporary_dir("target");
    fs::write(target.join("Cargo.toml"), "[package]\n").unwrap();

    let output = init(&state_home, &["scaf-mcp", target.to_str().unwrap()]);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Defaults to pytest."), "unconditional hints are always printed, stdout was: {stdout}");
    assert!(stdout.contains("Rust project detected: use cargo-adapter-runner."), "stdout was: {stdout}");
    assert!(!stdout.contains("JS/TS project detected"), "package.json is absent, stdout was: {stdout}");

    fs::remove_dir_all(project).unwrap();
    fs::remove_dir_all(target).unwrap();
}

#[cfg(unix)]
#[test]
fn init_copies_an_explicitly_mapped_file_even_when_exclude_lists_its_source_path() {
    // `exclude` only filters the `dirs` copies; a file named in `files` is always copied.
    let (project, state_home) = installed_scaffold_mcp();
    let target = temporary_dir("target");

    let output = init(&state_home, &["scaf-mcp", target.to_str().unwrap()]);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(fs::read_to_string(target.join(".mcp.json")).unwrap(), "{\"from\":\"template\"}\n");
    assert!(!target.join(".agents/private.txt").exists(), "dirs copies still honour exclude");

    fs::remove_dir_all(project).unwrap();
    fs::remove_dir_all(target).unwrap();
}

use std::{fs, path::PathBuf, process::{Command, Output}, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_dir() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!("mcpctl-doctor-checks-test-{}-{}", std::process::id(), NEXT_ID.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(&path).unwrap();
    path
}

/// Installs a one-file binary MCP through the real `install` command; returns the state home.
#[cfg(unix)]
fn install_binary_mcp() -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let project = temporary_dir();
    fs::write(project.join("mcpctl.toml"), "name = \"bin-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"bin-mcp\"\n").unwrap();
    fs::write(project.join("bin-mcp"), "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(project.join("bin-mcp"), fs::Permissions::from_mode(0o755)).unwrap();
    let state_home = project.join("state");
    let install = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["install", project.to_str().unwrap()]).env("MCPCTL_HOME", &state_home).output().unwrap();
    assert!(install.status.success(), "stderr was: {}", String::from_utf8_lossy(&install.stderr));
    (project, state_home)
}

fn doctor(state_home: &PathBuf) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mcp-cli")).arg("doctor").env("MCPCTL_HOME", state_home).output().unwrap()
}

#[cfg(unix)]
#[test]
fn doctor_warns_about_an_orphaned_package_directory_but_stays_healthy() {
    let (project, state_home) = install_binary_mcp();
    fs::create_dir_all(state_home.join("packages/ghost-mcp/9.9.9")).unwrap();

    let output = doctor(&state_home);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("WARN") && stdout.contains("ghost-mcp"), "{stdout}");
    assert!(output.status.success(), "a warning alone must not fail doctor: {stdout}");

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn doctor_errors_when_the_active_file_disagrees_with_the_registry() {
    let (project, state_home) = install_binary_mcp();
    fs::write(state_home.join("active/bin-mcp.json"), r#"{"schema_version":1,"name":"bin-mcp","version":"2.0.0"}"#).unwrap();

    let output = doctor(&state_home);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!output.status.success(), "{stdout}");
    assert!(stdout.contains("ERROR") && stdout.contains("active"), "{stdout}");

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn doctor_errors_when_the_data_directory_is_not_writable() {
    use std::os::unix::fs::PermissionsExt;
    let (project, state_home) = install_binary_mcp();
    fs::set_permissions(&state_home, fs::Permissions::from_mode(0o555)).unwrap();

    let output = doctor(&state_home);

    // Restore first so cleanup works even if an assertion fails.
    fs::set_permissions(&state_home, fs::Permissions::from_mode(0o755)).unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!output.status.success(), "{stdout}");
    assert!(stdout.contains("ERROR") && stdout.contains("writable"), "{stdout}");

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn doctor_reports_unreadable_metadata_without_panicking() {
    let (project, state_home) = install_binary_mcp();
    fs::write(state_home.join("packages/bin-mcp/1.0.0/metadata.json"), "{ not json").unwrap();

    let output = doctor(&state_home);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"), "doctor panicked");
    assert!(!output.status.success(), "{stdout}");
    assert!(stdout.contains("ERROR") && stdout.contains("metadata"), "{stdout}");

    fs::remove_dir_all(project).unwrap();
}

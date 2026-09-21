use std::{fs, path::PathBuf, process::Command, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_state() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!("mcpctl-management-test-{}-{}", std::process::id(), NEXT_ID.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(&path).unwrap(); path
}

#[test]
fn list_shows_installed_packages_active_versions_and_runtimes() {
    let state = temporary_state();
    fs::write(state.join("registry.json"), r#"{"schema_version":1,"packages":{"alpha":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"},{"version":"2.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).arg("list").env("MCPCTL_HOME", &state).output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("alpha") && text.contains("1.0.0") && text.contains("2.0.0") && text.contains("python"));
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn info_shows_installed_package_metadata_and_active_status() {
    let state = temporary_state();
    fs::write(state.join("registry.json"), r#"{"schema_version":1,"packages":{"alpha":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["info", "alpha"]).env("MCPCTL_HOME", &state).output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(text.contains("alpha") && text.contains("1.0.0") && text.contains("python") && text.contains("active"));
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn uninstall_removes_requested_non_active_version() {
    let state = temporary_state();
    fs::create_dir_all(state.join("packages/alpha/1.0.0")).unwrap(); fs::create_dir_all(state.join("packages/alpha/2.0.0")).unwrap();
    fs::write(state.join("registry.json"), r#"{"schema_version":1,"packages":{"alpha":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"},{"version":"2.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["uninstall", "alpha@2.0.0"]).env("MCPCTL_HOME", &state).output().unwrap();
    assert!(output.status.success());
    assert!(!state.join("packages/alpha/2.0.0").exists());
    assert_eq!(mcp_cli::registry::load_registry(&state.join("registry.json")).unwrap().packages["alpha"].versions.len(), 1);
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn uninstall_clears_sole_active_and_guards_active_with_alternatives() {
    let state = temporary_state(); fs::create_dir_all(state.join("packages/alpha/1.0.0")).unwrap();
    fs::write(state.join("registry.json"), r#"{"schema_version":1,"packages":{"alpha":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#).unwrap();
    assert!(Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["uninstall", "alpha@1.0.0"]).env("MCPCTL_HOME", &state).status().unwrap().success());
    assert!(mcp_cli::registry::load_registry(&state.join("registry.json")).unwrap().packages["alpha"].active_version.is_none());
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn uninstall_rejects_path_traversal_selector() {
    let state = temporary_state();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["uninstall", "../outside@1.0.0"]).env("MCPCTL_HOME", &state).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid package selector"));
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn list_uses_active_version_runtime() {
    let state = temporary_state();
    fs::write(state.join("registry.json"), r#"{"schema_version":1,"packages":{"alpha":{"active_version":"2.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"},{"version":"2.0.0","runtime":"binary","source":"local","installed_at":"now"}]}}}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).arg("list").env("MCPCTL_HOME", &state).output().unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("binary"));
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn use_persistently_selects_installed_version_and_rejects_unknown_versions() {
    let state = temporary_state();
    fs::write(state.join("registry.json"), r#"{"schema_version":1,"packages":{"alpha":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"},{"version":"2.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["use", "alpha@2.0.0"]).env("MCPCTL_HOME", &state).output().unwrap();
    assert!(output.status.success());
    assert_eq!(mcp_cli::registry::load_registry(&state.join("registry.json")).unwrap().packages["alpha"].active_version.as_deref(), Some("2.0.0"));
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["use", "alpha@3.0.0"]).env("MCPCTL_HOME", &state).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not installed"));
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn use_rejects_invalid_package_selector() {
    let state = temporary_state();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["use", "../alpha@1.0.0"]).env("MCPCTL_HOME", &state).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid package selector"));
    fs::remove_dir_all(state).unwrap();
}

#[cfg(unix)]
#[test]
fn uninstall_restores_registry_entry_when_directory_removal_fails() {
    use std::os::unix::fs::PermissionsExt;
    let state = temporary_state();
    let package_dir = state.join("packages/alpha");
    fs::create_dir_all(package_dir.join("1.0.0")).unwrap();
    fs::write(package_dir.join("1.0.0/marker"), "x").unwrap();
    let registry_path = state.join("registry.json");
    let original = r#"{"schema_version":1,"packages":{"alpha":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#;
    fs::write(&registry_path, original).unwrap();
    fs::set_permissions(&package_dir, fs::Permissions::from_mode(0o555)).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["uninstall", "alpha@1.0.0"]).env("MCPCTL_HOME", &state).output().unwrap();

    fs::set_permissions(&package_dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!output.status.success());
    assert!(package_dir.join("1.0.0").exists());
    let registry = mcp_cli::registry::load_registry(&registry_path).unwrap();
    assert_eq!(registry.packages["alpha"].versions.len(), 1, "registry entry lost while files remain");
    let retry = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["uninstall", "alpha@1.0.0"]).env("MCPCTL_HOME", &state).output().unwrap();
    assert!(retry.status.success(), "{}", String::from_utf8_lossy(&retry.stderr));
    assert!(!package_dir.join("1.0.0").exists());
    fs::remove_dir_all(state).unwrap();
}

#[cfg(unix)]
#[test]
fn uninstall_keeps_package_files_when_registry_save_fails() {
    use std::os::unix::fs::PermissionsExt;
    let state = temporary_state(); let package = state.join("packages/alpha/1.0.0"); fs::create_dir_all(&package).unwrap();
    let registry_path = state.join("registry.json"); fs::write(&registry_path, r#"{"schema_version":1,"packages":{"alpha":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#).unwrap();
    // The registry is replaced by rename, so a read-only file no longer blocks a save; an unwritable
    // state directory does (the temp file cannot be created). Pre-create the lock so locking succeeds.
    fs::write(state.join("mcpctl.lock"), "").unwrap();
    fs::set_permissions(&state, fs::Permissions::from_mode(0o555)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["uninstall", "alpha@1.0.0"]).env("MCPCTL_HOME", &state).output().unwrap();
    fs::set_permissions(&state, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!output.status.success()); assert!(package.exists());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Permission denied"), "{}", String::from_utf8_lossy(&output.stderr));
    fs::remove_dir_all(state).unwrap();
}

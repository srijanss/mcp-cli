use std::{fs, path::PathBuf, process::Command, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_project() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-atomic-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    fs::write(
        path.join("mcpctl.toml"),
        r#"name = "example-mcp"
version = "1.0.0"
[runtime]
type = "python"
python = ">=3.12"
[install]
strategy = "uv"
entrypoint = "example-mcp"
"#,
    )
    .unwrap();
    path
}

#[test]
fn install_fails_fast_while_another_process_holds_the_state_lock() {
    let project = temporary_project();
    let state_home = project.join("state");
    fs::create_dir_all(&state_home).unwrap();
    let lock = fs::File::create(state_home.join("mcpctl.lock")).unwrap();
    lock.lock().unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["install", project.to_str().unwrap()])
        .env("MCPCTL_HOME", &state_home)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("locked"),
        "stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!state_home.join("packages").exists(), "a locked install must not touch state");

    drop(lock);
    fs::remove_dir_all(project).unwrap();
}

#[test]
fn registry_is_replaced_by_rename_not_rewritten_in_place() {
    let project = temporary_project();
    let state_home = project.join("state");
    fs::create_dir_all(&state_home).unwrap();
    let registry = r#"{"schema_version":1,"packages":{"example-mcp":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"},{"version":"2.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#;
    fs::write(state_home.join("registry.json"), registry).unwrap();
    // A hard link shares the inode: an in-place rewrite changes it, a rename-replace does not.
    fs::hard_link(state_home.join("registry.json"), state_home.join("registry.before")).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["use", "example-mcp@2.0.0"])
        .env("MCPCTL_HOME", &state_home)
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    assert!(fs::read_to_string(state_home.join("registry.json")).unwrap().contains(r#""active_version": "2.0.0""#));
    assert_eq!(
        fs::read_to_string(state_home.join("registry.before")).unwrap(),
        registry,
        "registry.json was rewritten in place instead of replaced atomically"
    );

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn update_does_not_deadlock_against_its_own_install_step() {
    use std::os::unix::fs::PermissionsExt;
    let project = temporary_project();
    let state_home = project.join("state");
    let bin = project.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("uv"), "#!/bin/sh\nif [ \"$1\" = \"venv\" ]; then /bin/mkdir -p \"$2\"; fi\nexit 0\n").unwrap();
    fs::set_permissions(bin.join("uv"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        project.join("mcpctl.toml"),
        "name = \"example-mcp\"\nversion = \"2.0.0\"\n[runtime]\ntype = \"python\"\npython = \">=3.12\"\n[install]\nstrategy = \"uv\"\nentrypoint = \"example-mcp\"\n",
    )
    .unwrap();
    fs::create_dir_all(state_home.join("packages/example-mcp/1.0.0")).unwrap();
    fs::write(
        state_home.join("registry.json"),
        r#"{"schema_version":1,"packages":{"example-mcp":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["update", "example-mcp", "--source", project.to_str().unwrap()])
        .env("MCPCTL_HOME", &state_home)
        .env("PATH", &bin)
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));

    fs::remove_dir_all(project).unwrap();
}

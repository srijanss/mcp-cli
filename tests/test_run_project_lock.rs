use std::{fs, path::{Path, PathBuf}, process::{Command, Output}, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_dir() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-run-project-lock-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn install_fixture(state_home: &Path, version: &str, output: &str) {
    let package = state_home.join("packages/example-mcp").join(version);
    let bin = package.join("runtime/.venv/bin");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(package.join("source")).unwrap();
    fs::write(package.join("source/mcpctl.toml"), format!("name = \"example-mcp\"\nversion = \"{version}\"\n[runtime]\ntype = \"python\"\npython = \">=3.12\"\n[install]\nstrategy = \"uv\"\nentrypoint = \"example-mcp\"\n")).unwrap();
    fs::write(bin.join("example-mcp"), format!("#!/bin/sh\nprintf '{output}'\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(bin.join("example-mcp"), fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// A state home with example-mcp 1.0.0 (active), 2.0.0 and 3.0.0 installed.
fn state_with_three_versions() -> PathBuf {
    let state_home = temporary_dir();
    install_fixture(&state_home, "1.0.0", "one");
    install_fixture(&state_home, "2.0.0", "two");
    install_fixture(&state_home, "3.0.0", "three");
    fs::write(
        state_home.join("registry.json"),
        r#"{"schema_version":1,"packages":{"example-mcp":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"},{"version":"2.0.0","runtime":"python","source":"local","installed_at":"now"},{"version":"3.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#,
    )
    .unwrap();
    state_home
}

/// A project directory whose `.mcpctl.lock` pins `mcp_name` to `version`.
fn project_pinning(mcp_name: &str, version: &str) -> PathBuf {
    let root = temporary_dir();
    fs::write(
        root.join(".mcpctl.toml"),
        format!("[project]\nname = \"service\"\n\n[[mcp]]\nname = \"{mcp_name}\"\nversion = \"={version}\"\nsource = \"../{mcp_name}\"\n"),
    )
    .unwrap();
    fs::write(
        root.join(".mcpctl.lock"),
        format!("version = 1\n\n[[mcp]]\nname = \"{mcp_name}\"\nversion = \"{version}\"\nsource = \"../{mcp_name}\"\nmanifest_digest = \"sha256:abc123\"\n"),
    )
    .unwrap();
    root
}

fn run_in(directory: &Path, state_home: &Path, selector: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["run", selector])
        .current_dir(directory)
        .env("MCPCTL_HOME", state_home)
        .output()
        .unwrap()
}

#[test]
fn run_inside_a_project_uses_the_locked_version_and_falls_back_to_active_elsewhere() {
    let state_home = state_with_three_versions();
    let first_project = project_pinning("example-mcp", "2.0.0");
    let second_project = project_pinning("example-mcp", "3.0.0");
    let unrelated_project = project_pinning("other-mcp", "9.9.9");
    let nested = first_project.join("src/deeply/nested");
    fs::create_dir_all(&nested).unwrap();
    let outside = temporary_dir();

    let from_nested = run_in(&nested, &state_home, "example-mcp");
    let from_second = run_in(&second_project, &state_home, "example-mcp");
    let explicit = run_in(&second_project, &state_home, "example-mcp@1.0.0");
    let not_locked = run_in(&unrelated_project, &state_home, "example-mcp");
    let from_outside = run_in(&outside, &state_home, "example-mcp");

    assert_eq!(String::from_utf8_lossy(&from_nested.stdout), "two", "{}", String::from_utf8_lossy(&from_nested.stderr));
    assert_eq!(String::from_utf8_lossy(&from_second.stdout), "three");
    assert_eq!(String::from_utf8_lossy(&explicit.stdout), "one");
    assert_eq!(String::from_utf8_lossy(&not_locked.stdout), "one");
    assert_eq!(String::from_utf8_lossy(&from_outside.stdout), "one");
    assert_eq!(
        mcp_cli::registry::load_registry(&state_home.join("registry.json")).unwrap().packages["example-mcp"].active_version,
        Some("1.0.0".to_owned())
    );
}

#[test]
fn run_fails_naming_the_project_when_the_locked_version_is_not_installed() {
    let state_home = state_with_three_versions();
    let project = project_pinning("example-mcp", "4.0.0");

    let output = run_in(&project, &state_home, "example-mcp");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "locked-version errors must not fall back to another version");
    assert!(stderr.contains("example-mcp@4.0.0"), "{stderr}");
    assert!(stderr.contains(&project.join(".mcpctl.lock").display().to_string()), "{stderr}");
}

#[test]
fn run_fails_naming_the_lock_file_when_the_project_lock_is_malformed() {
    let state_home = state_with_three_versions();
    let project = project_pinning("example-mcp", "2.0.0");
    fs::write(project.join(".mcpctl.lock"), "version = 1\n[[mcp]\nbroken").unwrap();

    let output = run_in(&project, &state_home, "example-mcp");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "a malformed lock must not fall back to the active version");
    assert!(stderr.contains(&project.join(".mcpctl.lock").display().to_string()), "{stderr}");
}

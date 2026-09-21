use std::{fs, io::Write, path::PathBuf, process::{Command, Stdio}, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_state() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-run-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn install_fixture(state_home: &std::path::Path, version: &str, output: &str) {
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

#[test]
fn run_uses_active_installed_version() {
    let state_home = temporary_state();
    install_fixture(&state_home, "1.0.0", "active");
    install_fixture(&state_home, "2.0.0", "inactive");
    fs::write(
        state_home.join("registry.json"),
        r#"{"schema_version":1,"packages":{"example-mcp":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"},{"version":"2.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["run", "example-mcp"])
        .env("MCPCTL_HOME", &state_home)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(output.stdout, b"active");
    assert!(output.stderr.is_empty());
    fs::remove_dir_all(state_home).unwrap();
}

#[test]
fn run_uses_explicit_installed_version_without_changing_active_selection() {
    let state_home = temporary_state();
    install_fixture(&state_home, "1.0.0", "active");
    install_fixture(&state_home, "2.0.0", "selected");
    fs::write(
        state_home.join("registry.json"),
        r#"{"schema_version":1,"packages":{"example-mcp":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"},{"version":"2.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["run", "example-mcp@2.0.0"])
        .env("MCPCTL_HOME", &state_home)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(output.stdout, b"selected");
    assert_eq!(
        mcp_cli::registry::load_registry(&state_home.join("registry.json")).unwrap().packages["example-mcp"].active_version,
        Some("1.0.0".to_owned())
    );
    fs::remove_dir_all(state_home).unwrap();
}

#[test]
fn run_forwards_arguments_stdout_stderr_and_exit_status() {
    let state_home = temporary_state();
    install_fixture(&state_home, "1.0.0", "");
    fs::write(
        state_home.join("packages/example-mcp/1.0.0/runtime/.venv/bin/example-mcp"),
        "#!/bin/sh\nprintf 'child:%s' \"$1\"\nprintf 'child error' >&2\nexit 7\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            state_home.join("packages/example-mcp/1.0.0/runtime/.venv/bin/example-mcp"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    fs::write(
        state_home.join("registry.json"),
        r#"{"schema_version":1,"packages":{"example-mcp":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["run", "example-mcp", "hello"])
        .env("MCPCTL_HOME", &state_home)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout, b"child:hello");
    assert_eq!(output.stderr, b"child error");
    fs::remove_dir_all(state_home).unwrap();
}

#[test]
fn run_forwards_stdin_to_child() {
    let state_home = temporary_state();
    install_fixture(&state_home, "1.0.0", "");
    fs::write(
        state_home.join("packages/example-mcp/1.0.0/runtime/.venv/bin/example-mcp"),
        "#!/bin/sh\ncat\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            state_home.join("packages/example-mcp/1.0.0/runtime/.venv/bin/example-mcp"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    fs::write(
        state_home.join("registry.json"),
        r#"{"schema_version":1,"packages":{"example-mcp":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#,
    )
    .unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["run", "example-mcp"])
        .env("MCPCTL_HOME", &state_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"handshake").unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    assert_eq!(output.stdout, b"handshake");
    fs::remove_dir_all(state_home).unwrap();
}

#[test]
fn run_preserves_caller_context_and_clears_python_virtual_environment() {
    let state_home = temporary_state();
    let working_directory = state_home.join("caller-directory");
    fs::create_dir(&working_directory).unwrap();
    install_fixture(&state_home, "1.0.0", "");
    fs::write(
        state_home.join("packages/example-mcp/1.0.0/runtime/.venv/bin/example-mcp"),
        "#!/bin/sh\nprintf '%s|%s|%s|%s' \"$PWD\" \"$CALLER_CONTEXT\" \"${VIRTUAL_ENV-unset}\" \"${PYTHONHOME-unset}\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(state_home.join("packages/example-mcp/1.0.0/runtime/.venv/bin/example-mcp"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::write(state_home.join("registry.json"), r#"{"schema_version":1,"packages":{"example-mcp":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["run", "example-mcp"])
        .env("MCPCTL_HOME", &state_home)
        .env("CALLER_CONTEXT", "retained")
        .env("VIRTUAL_ENV", "/wrong/venv")
        .env("PYTHONHOME", "/wrong/python")
        .current_dir(&working_directory)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(output.stdout, format!("{}|retained|unset|unset", working_directory.canonicalize().unwrap().display()).as_bytes());
    fs::remove_dir_all(state_home).unwrap();
}

#[test]
fn run_does_not_inherit_virtual_env() {
    let state_home = temporary_state();
    install_fixture(&state_home, "1.0.0", "");
    fs::write(state_home.join("packages/example-mcp/1.0.0/runtime/.venv/bin/example-mcp"), "#!/bin/sh\nprintf '%s' \"${VIRTUAL_ENV-set}\"\n").unwrap();
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(state_home.join("packages/example-mcp/1.0.0/runtime/.venv/bin/example-mcp"), fs::Permissions::from_mode(0o755)).unwrap(); }
    fs::write(state_home.join("registry.json"), r#"{"schema_version":1,"packages":{"example-mcp":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["run", "example-mcp"]).env("MCPCTL_HOME", &state_home).env("VIRTUAL_ENV", "/wrong/venv").output().unwrap();
    assert_eq!(output.stdout, b"set");
    fs::remove_dir_all(state_home).unwrap();
}

#[test]
fn run_rejects_empty_explicit_version_with_clear_diagnostic() {
    let state_home = temporary_state();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["run", "example-mcp@"]) 
        .env("MCPCTL_HOME", &state_home)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("version is required"));
    assert!(output.stdout.is_empty());
    fs::remove_dir_all(state_home).unwrap();
}

#[test]
fn run_surfaces_corrupt_registry_instead_of_reporting_not_installed() {
    let state_home = temporary_state();
    fs::write(state_home.join("registry.json"), "{ invalid json").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["run", "example-mcp"])
        .env("MCPCTL_HOME", &state_home)
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("invalid registry"), "{stderr}");
    assert!(!stderr.contains("not installed"), "{stderr}");
    assert_eq!(fs::read_to_string(state_home.join("registry.json")).unwrap(), "{ invalid json");
    fs::remove_dir_all(state_home).unwrap();
}

#[test]
fn run_reports_not_installed_when_registry_is_missing() {
    let state_home = temporary_state();

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["run", "example-mcp"])
        .env("MCPCTL_HOME", &state_home)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not installed"));
    fs::remove_dir_all(state_home).unwrap();
}

#[test]
fn run_reports_missing_active_version() {
    let state_home = temporary_state();
    fs::write(state_home.join("registry.json"), r#"{"schema_version":1,"packages":{"example-mcp":{"active_version":null,"versions":[]}}}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["run", "example-mcp"]).env("MCPCTL_HOME", &state_home).output().unwrap();
    assert!(String::from_utf8_lossy(&output.stderr).contains("no active version"));
    fs::remove_dir_all(state_home).unwrap();
}

#[test]
fn run_uses_manifest_entrypoint_instead_of_package_name() {
    let state_home = temporary_state();
    let package = state_home.join("packages/example-mcp/1.0.0");
    fs::create_dir_all(package.join("runtime/.venv/bin")).unwrap();
    fs::create_dir_all(package.join("source")).unwrap();
    fs::write(package.join("source/mcpctl.toml"), "name = \"example-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"python\"\npython = \">=3.12\"\n[install]\nstrategy = \"uv\"\nentrypoint = \"custom-server\"\n").unwrap();
    fs::write(package.join("runtime/.venv/bin/custom-server"), "#!/bin/sh\nprintf custom\n").unwrap();
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(package.join("runtime/.venv/bin/custom-server"), fs::Permissions::from_mode(0o755)).unwrap(); }
    fs::write(state_home.join("registry.json"), r#"{"schema_version":1,"packages":{"example-mcp":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["run", "example-mcp"]).env("MCPCTL_HOME", &state_home).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"custom");
    fs::remove_dir_all(state_home).unwrap();
}

#[cfg(unix)]
#[test]
fn installed_binary_with_nested_entrypoint_installs_and_runs() {
    use std::os::unix::fs::PermissionsExt;
    let project = temporary_state();
    let state_home = project.join("state");
    fs::write(project.join("mcpctl.toml"), "name = \"nested-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"dist/nested-mcp\"\n").unwrap();
    fs::create_dir_all(project.join("dist")).unwrap();
    fs::write(project.join("dist/nested-mcp"), "#!/bin/sh\nprintf nested\n").unwrap();
    fs::set_permissions(project.join("dist/nested-mcp"), fs::Permissions::from_mode(0o755)).unwrap();

    let install = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["install", project.to_str().unwrap()]).env("MCPCTL_HOME", &state_home).output().unwrap();
    assert!(install.status.success(), "{}", String::from_utf8_lossy(&install.stderr));
    let run = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["run", "nested-mcp"]).env("MCPCTL_HOME", &state_home).output().unwrap();

    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    assert_eq!(run.stdout, b"nested");
    fs::remove_dir_all(project).unwrap();
}

#[test]
fn run_rejects_path_components_in_package_selector() {
    let state_home = temporary_state();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["run", "../other@1.0.0"]).env("MCPCTL_HOME", &state_home).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid package selector"));
    fs::remove_dir_all(state_home).unwrap();
}

#[test]
fn run_rejects_path_separator_in_version() {
    let state_home = temporary_state();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["run", "example-mcp@../other"]).env("MCPCTL_HOME", &state_home).output().unwrap();
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid package selector"));
    fs::remove_dir_all(state_home).unwrap();
}

#[test]
fn run_prepends_selected_runtime_bin_to_path() {
    let state_home = temporary_state();
    install_fixture(&state_home, "1.0.0", "");
    fs::write(state_home.join("packages/example-mcp/1.0.0/runtime/.venv/bin/example-mcp"), "#!/bin/sh\nhelper\n").unwrap();
    fs::write(state_home.join("packages/example-mcp/1.0.0/runtime/.venv/bin/helper"), "#!/bin/sh\nprintf helper-found\n").unwrap();
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; for file in ["example-mcp", "helper"] { fs::set_permissions(state_home.join("packages/example-mcp/1.0.0/runtime/.venv/bin").join(file), fs::Permissions::from_mode(0o755)).unwrap(); } }
    fs::write(state_home.join("registry.json"), r#"{"schema_version":1,"packages":{"example-mcp":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["run", "example-mcp"]).env("MCPCTL_HOME", &state_home).env("PATH", "/usr/bin:/bin").output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"helper-found");
    fs::remove_dir_all(state_home).unwrap();
}

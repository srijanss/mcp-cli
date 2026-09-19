use std::{fs, path::PathBuf, process::Command, sync::atomic::{AtomicUsize, Ordering}};

fn state() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!("mcpctl-doctor-test-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(path.join("packages/binary-mcp/1.0.0/runtime/bin")).unwrap();
    fs::create_dir_all(path.join("packages/binary-mcp/1.0.0/source")).unwrap();
    fs::write(path.join("registry.json"), r#"{"schema_version":1,"packages":{"binary-mcp":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"binary","source":"local","installed_at":"now"}]}}}"#).unwrap();
    path
}

#[test]
fn doctor_reports_ok_for_a_healthy_binary_installation() {
    let state = state();
    fs::write(state.join("packages/binary-mcp/1.0.0/source/mcpctl.toml"), "name = \"binary-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"binary-mcp\"\n").unwrap();
    fs::write(state.join("packages/binary-mcp/1.0.0/runtime/bin/binary-mcp"), "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(state.join("packages/binary-mcp/1.0.0/runtime/bin/binary-mcp"), fs::Permissions::from_mode(0o755)).unwrap(); }
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).arg("doctor").env("MCPCTL_HOME", &state).output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("OK"));
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn doctor_reports_error_for_missing_entrypoint() {
    let state = state();
    fs::write(state.join("packages/binary-mcp/1.0.0/source/mcpctl.toml"), "name = \"binary-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"binary-mcp\"\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).arg("doctor").env("MCPCTL_HOME", &state).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("ERROR"));
    fs::remove_dir_all(state).unwrap();
}

#[test]
fn doctor_accepts_python_virtual_environment_without_package_entrypoint() {
    let state = state();
    fs::write(state.join("registry.json"), r#"{"schema_version":1,"packages":{"python-mcp":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#).unwrap();
    let root = state.join("packages/python-mcp/1.0.0"); fs::create_dir_all(root.join("source")).unwrap(); fs::create_dir_all(root.join("runtime/bin")).unwrap();
    fs::write(root.join("source/mcpctl.toml"), "name = \"python-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"python\"\npython = \">=3.12\"\n[install]\nstrategy = \"uv\"\nentrypoint = \"python-mcp\"\n").unwrap();
    fs::write(root.join("runtime/bin/python"), "#!/bin/sh\n").unwrap();
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(root.join("runtime/bin/python"), fs::Permissions::from_mode(0o755)).unwrap(); }
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).arg("doctor").env("MCPCTL_HOME", &state).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stdout)); fs::remove_dir_all(state).unwrap();
}

#[test]
fn doctor_reports_error_for_missing_python_interpreter() {
    let state = state();
    fs::write(state.join("registry.json"), r#"{"schema_version":1,"packages":{"python-mcp":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#).unwrap();
    let root = state.join("packages/python-mcp/1.0.0"); fs::create_dir_all(root.join("source")).unwrap();
    fs::write(root.join("source/mcpctl.toml"), "name = \"python-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"python\"\npython = \">=3.12\"\n[install]\nstrategy = \"uv\"\nentrypoint = \"python-mcp\"\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).arg("doctor").env("MCPCTL_HOME", &state).output().unwrap();
    assert!(!output.status.success()); assert!(String::from_utf8_lossy(&output.stdout).contains("ERROR")); fs::remove_dir_all(state).unwrap();
}

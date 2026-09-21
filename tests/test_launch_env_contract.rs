use std::{fs, path::PathBuf, process::Command, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_state() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!("mcpctl-launch-env-test-{}-{}", std::process::id(), NEXT_ID.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(&path).unwrap();
    path
}

/// Installed Python package whose manifest and `metadata.json` disagree about the entrypoint,
/// so a test can tell which of the two `run` resolved it from. Only `bin_name` exists on disk.
#[cfg(unix)]
fn install_fixture(state_home: &std::path::Path, manifest_entrypoint: &str, metadata_entrypoint: &str, bin_name: &str, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    let package = state_home.join("packages/example-mcp/1.0.0");
    let bin = package.join("runtime/.venv/bin");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(package.join("source")).unwrap();
    fs::write(package.join("source/mcpctl.toml"), format!("name = \"example-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"python\"\npython = \">=3.12\"\n[install]\nstrategy = \"uv\"\nentrypoint = \"{manifest_entrypoint}\"\n")).unwrap();
    fs::write(package.join("metadata.json"), format!("{{\"schema_version\":1,\"name\":\"example-mcp\",\"version\":\"1.0.0\",\"runtime_type\":\"python\",\"entrypoint_relative_path\":\"{metadata_entrypoint}\"}}")).unwrap();
    fs::write(bin.join(bin_name), script).unwrap();
    fs::set_permissions(bin.join(bin_name), fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(state_home.join("registry.json"), r#"{"schema_version":1,"packages":{"example-mcp":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#).unwrap();
}

#[cfg(unix)]
#[test]
fn run_resolves_the_entrypoint_from_metadata_not_the_manifest() {
    let state_home = temporary_state();
    install_fixture(&state_home, "manifest-name", "metadata-name", "metadata-name", "#!/bin/sh\nprintf from-metadata\n");

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["run", "example-mcp"]).env("MCPCTL_HOME", &state_home).output().unwrap();

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(output.stdout, b"from-metadata");
    fs::remove_dir_all(state_home).unwrap();
}

#[cfg(unix)]
#[test]
fn run_preserves_pythonpath_while_clearing_virtual_env_and_pythonhome() {
    let state_home = temporary_state();
    install_fixture(&state_home, "example-mcp", "example-mcp", "example-mcp", "#!/bin/sh\nprintf '%s|%s|%s' \"${PYTHONPATH-unset}\" \"${VIRTUAL_ENV-unset}\" \"${PYTHONHOME-unset}\"\n");

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["run", "example-mcp"])
        .env("MCPCTL_HOME", &state_home)
        .env("PYTHONPATH", "/caller/libs")
        .env("VIRTUAL_ENV", "/wrong/venv")
        .env("PYTHONHOME", "/wrong/python")
        .output()
        .unwrap();

    assert_eq!(output.stdout, b"/caller/libs|unset|unset");
    fs::remove_dir_all(state_home).unwrap();
}

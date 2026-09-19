use std::{fs, path::PathBuf, process::Command, sync::atomic::{AtomicUsize, Ordering}};

use mcp_cli::registry::load_registry;

fn temporary_project() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-install-test-{}-{}",
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
fn python_install_reports_clear_error_when_uv_is_unavailable() {
    let project = temporary_project();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .arg("install")
        .arg(&project)
        .env("PATH", "")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("uv"));

    fs::remove_dir_all(project).unwrap();
}

#[test]
fn python_install_rejects_uv_with_failed_version_probe() {
    let project = temporary_project();
    let state_home = project.join("state");
    let bin = project.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("uv"), "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(bin.join("uv"), fs::Permissions::from_mode(0o755)).unwrap();
    }

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["install", project.to_str().unwrap()])
        .env("MCPCTL_HOME", &state_home)
        .env("PATH", &bin)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("uv is required"));
    assert!(!state_home.exists());

    fs::remove_dir_all(project).unwrap();
}

#[test]
fn unsupported_runtime_is_rejected_without_snapshot() {
    let project = temporary_project();
    let state_home = project.join("state");
    fs::write(
        project.join("mcpctl.toml"),
        "name = \"binary-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"binary-mcp\"\n",
    )
    .unwrap();
    let bin = project.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("uv"), "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(bin.join("uv"), fs::Permissions::from_mode(0o755)).unwrap();
    }

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["install", project.to_str().unwrap()])
        .env("MCPCTL_HOME", &state_home)
        .env("PATH", &bin)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Python MCPs"));
    assert!(!state_home.join("packages/binary-mcp/1.0.0").exists());

    fs::remove_dir_all(project).unwrap();
}

#[test]
fn registry_failure_rolls_back_installed_version() {
    let project = temporary_project();
    let state_home = project.join("state");
    fs::create_dir_all(&state_home).unwrap();
    fs::write(state_home.join("registry.json"), "{ invalid").unwrap();
    let bin = project.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("uv"), "#!/bin/sh\nif [ \"$1\" = \"venv\" ]; then /bin/mkdir -p \"$2\"; fi\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(bin.join("uv"), fs::Permissions::from_mode(0o755)).unwrap();
    }

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["install", project.to_str().unwrap()])
        .env("MCPCTL_HOME", &state_home)
        .env("PATH", &bin)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_eq!(fs::read_to_string(state_home.join("registry.json")).unwrap(), "{ invalid");
    assert!(!state_home.join("packages/example-mcp/1.0.0").exists());

    fs::remove_dir_all(project).unwrap();
}

#[test]
fn install_command_requires_a_local_project_path() {
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .arg("install")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("project path"));
}

#[test]
fn python_install_copies_safe_source_snapshot() {
    let project = temporary_project();
    let state_home = project.join("state");
    let bin = project.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("uv"), "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(bin.join("uv"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::write(project.join("server.py"), "print('safe')\n").unwrap();
    fs::create_dir(project.join("__pycache__")).unwrap();
    fs::write(project.join("__pycache__/server.pyc"), "generated").unwrap();
    fs::create_dir(project.join(".git")).unwrap();
    fs::write(project.join(".git/config"), "private").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .arg("install")
        .arg(&project)
        .env("MCPCTL_HOME", &state_home)
        .env("PATH", &bin)
        .output()
        .unwrap();

    assert!(output.status.success());
    let snapshot = state_home.join("packages/example-mcp/1.0.0/source");
    assert_eq!(fs::read_to_string(snapshot.join("server.py")).unwrap(), "print('safe')\n");
    assert!(!snapshot.join("__pycache__").exists());
    assert!(!snapshot.join(".git").exists());

    fs::remove_dir_all(project).unwrap();
}

#[test]
fn source_snapshot_excludes_generated_and_metadata_directories() {
    let project = temporary_project();
    let state_home = project.join("state");
    let bin = project.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("uv"), "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(bin.join("uv"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::create_dir(project.join(".venv")).unwrap();
    fs::write(project.join(".venv/python"), "generated").unwrap();

    assert!(Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["install", project.to_str().unwrap()])
        .env("MCPCTL_HOME", &state_home)
        .env("PATH", &bin)
        .status()
        .unwrap()
        .success());
    assert!(!state_home
        .join("packages/example-mcp/1.0.0/source/.venv")
        .exists());

    fs::remove_dir_all(project).unwrap();
}

#[test]
fn python_install_creates_uv_isolated_runtime_outside_project() {
    let project = temporary_project();
    let state_home = project.join("state");
    let bin = project.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(
        bin.join("uv"),
        "#!/bin/sh\nif [ \"$1\" = \"venv\" ]; then /bin/mkdir -p \"$2\"; fi\nexit 0\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(bin.join("uv"), fs::Permissions::from_mode(0o755)).unwrap();
    }

    assert!(Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["install", project.to_str().unwrap()])
        .env("MCPCTL_HOME", &state_home)
        .env("PATH", &bin)
        .status()
        .unwrap()
        .success());
    assert!(state_home
        .join("packages/example-mcp/1.0.0/runtime")
        .is_dir());
    assert!(!project.join(".venv").exists());

    fs::remove_dir_all(project).unwrap();
}

#[test]
fn python_install_records_versions_and_keeps_first_active() {
    let project = temporary_project();
    let state_home = project.join("state");
    let bin = project.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("uv"), "#!/bin/sh\nif [ \"$1\" = \"venv\" ]; then /bin/mkdir -p \"$2\"; fi\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(bin.join("uv"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    for version in ["1.0.0", "2.0.0"] {
        fs::write(
            project.join("mcpctl.toml"),
            format!("name = \"example-mcp\"\nversion = \"{version}\"\n[runtime]\ntype = \"python\"\npython = \">=3.12\"\n[install]\nstrategy = \"uv\"\nentrypoint = \"example-mcp\"\n"),
        ).unwrap();
        assert!(Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
            .args(["install", project.to_str().unwrap()])
            .env("MCPCTL_HOME", &state_home)
            .env("PATH", &bin)
            .status().unwrap().success());
    }
    let record = &load_registry(&state_home.join("registry.json")).unwrap().packages["example-mcp"];
    assert_eq!(record.active_version.as_deref(), Some("1.0.0"));
    assert_eq!(record.versions.len(), 2);

    fs::remove_dir_all(project).unwrap();
}

#[test]
fn failed_python_install_leaves_no_installed_or_active_state() {
    let project = temporary_project();
    let state_home = project.join("state");
    let bin = project.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(
        bin.join("uv"),
        "#!/bin/sh\nif [ \"$1\" = \"venv\" ]; then exit 7; fi\nexit 0\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(bin.join("uv"), fs::Permissions::from_mode(0o755)).unwrap();
    }

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["install", project.to_str().unwrap()])
        .env("MCPCTL_HOME", &state_home)
        .env("PATH", &bin)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("uv"));
    assert!(!state_home.join("packages/example-mcp/1.0.0").exists());
    assert!(!state_home.join("registry.json").exists());

    fs::remove_dir_all(project).unwrap();
}

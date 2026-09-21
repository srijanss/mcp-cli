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
fn python_install_installs_snapshot_dependencies_into_its_isolated_runtime() {
    let project = temporary_project();
    let state_home = project.join("state");
    let bin = project.join("bin");
    let log = project.join("uv.log");
    fs::create_dir_all(&bin).unwrap();
    fs::write(
        bin.join("uv"),
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$UV_LOG\"\nif [ \"$1\" = \"venv\" ]; then /bin/mkdir -p \"$2/bin\"; /usr/bin/touch \"$2/bin/python\"; fi\nexit 0\n",
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
        .env("UV_LOG", &log)
        .env("PATH", &bin)
        .status()
        .unwrap()
        .success());

    let runtime = state_home.join("packages/example-mcp/1.0.0/runtime/bin/python");
    let snapshot = state_home.join("packages/example-mcp/1.0.0/source");
    let log_contents = fs::read_to_string(log).unwrap();
    assert!(log_contents.contains(&format!("pip install --python {} {}", runtime.display(), snapshot.display())));
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

#[test]
fn pip_install_failure_removes_all_package_state() {
    let project = temporary_project();
    let state_home = project.join("state");
    let bin = project.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("uv"), "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then exit 0; fi\nif [ \"$1\" = \"venv\" ]; then /bin/mkdir -p \"$2/bin\"; exit 0; fi\nexit 7\n").unwrap();
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(bin.join("uv"), fs::Permissions::from_mode(0o755)).unwrap(); }
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["install", project.to_str().unwrap()]).env("MCPCTL_HOME", &state_home).env("PATH", &bin).output().unwrap();
    assert!(!output.status.success());
    assert!(!state_home.join("packages/example-mcp").exists());
    assert!(!state_home.join("registry.json").exists());
    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn binary_install_copies_executable_without_uv_and_records_binary_runtime() {
    use std::os::unix::fs::PermissionsExt;
    let project = temporary_project();
    let state_home = project.join("state");
    fs::write(project.join("mcpctl.toml"), "name = \"binary-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"binary-mcp\"\n").unwrap();
    fs::write(project.join("binary-mcp"), "#!/bin/sh\nprintf binary\n").unwrap();
    fs::set_permissions(project.join("binary-mcp"), fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["install", project.to_str().unwrap()]).env("MCPCTL_HOME", &state_home).env("PATH", "").output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(state_home.join("packages/binary-mcp/1.0.0/runtime/bin/binary-mcp").is_file());
    assert_eq!(load_registry(&state_home.join("registry.json")).unwrap().packages["binary-mcp"].versions[0].runtime, "binary");
    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn binary_install_rejects_non_executable_entrypoint() {
    let project = temporary_project();
    fs::write(project.join("mcpctl.toml"), "name = \"binary-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"binary-mcp\"\n").unwrap();
    fs::write(project.join("binary-mcp"), "not executable").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["install", project.to_str().unwrap()]).env("MCPCTL_HOME", project.join("state")).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("executable"));
    fs::remove_dir_all(project).unwrap();
}

#[test]
fn binary_install_rejects_traversal_entrypoint() {
    let project = temporary_project();
    fs::write(project.join("mcpctl.toml"), "name = \"binary-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"../outside\"\n").unwrap();
    fs::write(project.parent().unwrap().join("outside"), "#!/bin/sh\n").unwrap();
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(project.parent().unwrap().join("outside"), fs::Permissions::from_mode(0o755)).unwrap(); }
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["install", project.to_str().unwrap()]).env("MCPCTL_HOME", project.join("state")).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("entrypoint"));
    let _ = fs::remove_file(project.parent().unwrap().join("outside")); fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn binary_install_accepts_source_relative_entrypoint_path() {
    use std::os::unix::fs::PermissionsExt;
    let project = temporary_project();
    let state_home = project.join("state");
    fs::write(project.join("mcpctl.toml"), "name = \"example-rust-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"dist/example-rust-mcp\"\n").unwrap();
    fs::create_dir_all(project.join("dist")).unwrap();
    fs::write(project.join("dist/example-rust-mcp"), "#!/bin/sh\nprintf binary\n").unwrap();
    fs::set_permissions(project.join("dist/example-rust-mcp"), fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["install", project.to_str().unwrap()]).env("MCPCTL_HOME", &state_home).env("PATH", "").output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(state_home.join("packages/example-rust-mcp/1.0.0/runtime/bin/dist/example-rust-mcp").is_file());
    fs::remove_dir_all(project).unwrap();
}

#[test]
fn binary_install_rejects_absolute_entrypoint_path() {
    let project = temporary_project();
    fs::write(project.join("mcpctl.toml"), "name = \"binary-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"/bin/sh\"\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["install", project.to_str().unwrap()]).env("MCPCTL_HOME", project.join("state")).output().unwrap();
    assert!(!output.status.success()); assert!(String::from_utf8_lossy(&output.stderr).contains("entrypoint"));
    assert!(!project.join("state/packages").exists());
    fs::remove_dir_all(project).unwrap();
}

#[test]
fn binary_install_rejects_missing_nested_entrypoint_file() {
    let project = temporary_project();
    fs::write(project.join("mcpctl.toml"), "name = \"binary-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"bin/server\"\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["install", project.to_str().unwrap()]).output().unwrap();
    assert!(!output.status.success()); assert!(String::from_utf8_lossy(&output.stderr).contains("entrypoint"));
    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn binary_install_rejects_mode_zero_entrypoint() {
    use std::os::unix::fs::PermissionsExt;
    let project = temporary_project();
    fs::write(project.join("mcpctl.toml"), "name = \"binary-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"binary-mcp\"\n").unwrap();
    fs::write(project.join("binary-mcp"), "#!/bin/sh\n").unwrap(); fs::set_permissions(project.join("binary-mcp"), fs::Permissions::from_mode(0o000)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["install", project.to_str().unwrap()]).env("MCPCTL_HOME", project.join("state")).output().unwrap();
    assert!(!output.status.success()); assert!(String::from_utf8_lossy(&output.stderr).contains("executable"));
    fs::set_permissions(project.join("binary-mcp"), fs::Permissions::from_mode(0o644)).unwrap(); fs::remove_dir_all(project).unwrap();
}

#[test]
fn update_installs_new_local_version_and_selects_it_after_success() {
    let project = temporary_project(); let state_home = project.join("state"); let bin = project.join("bin"); fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("uv"), "#!/bin/sh\nif [ \"$1\" = \"venv\" ]; then /bin/mkdir -p \"$2\"; fi\nexit 0\n").unwrap();
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(bin.join("uv"), fs::Permissions::from_mode(0o755)).unwrap(); }
    fs::write(project.join("mcpctl.toml"), "name = \"example-mcp\"\nversion = \"2.0.0\"\n[runtime]\ntype = \"python\"\npython = \">=3.12\"\n[install]\nstrategy = \"uv\"\nentrypoint = \"example-mcp\"\n").unwrap();
    fs::create_dir_all(state_home.join("packages/example-mcp/1.0.0")).unwrap();
    fs::write(state_home.join("registry.json"), r#"{"schema_version":1,"packages":{"example-mcp":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["update", "example-mcp", "--source", project.to_str().unwrap()]).env("MCPCTL_HOME", &state_home).env("PATH", &bin).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let record = &load_registry(&state_home.join("registry.json")).unwrap().packages["example-mcp"];
    assert_eq!(record.active_version.as_deref(), Some("2.0.0")); assert_eq!(record.versions.len(), 2);
    fs::remove_dir_all(project).unwrap();
}

#[test]
fn update_rejects_a_source_for_a_different_package() {
    let project = temporary_project();
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["update", "other", "--source", project.to_str().unwrap()]).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("does not match"));
    fs::remove_dir_all(project).unwrap();
}

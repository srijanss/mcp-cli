use std::{fs, path::PathBuf, process::Command, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_project() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-uvsync-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    fs::write(
        path.join("mcpctl.toml"),
        "name = \"example-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"python\"\npython = \">=3.12\"\n[install]\nstrategy = \"uv\"\nentrypoint = \"example-mcp\"\n",
    )
    .unwrap();
    fs::write(path.join("pyproject.toml"), "[project]\nname = \"example-mcp\"\nversion = \"1.0.0\"\n").unwrap();
    fs::write(path.join("uv.lock"), "version = 1\n").unwrap();
    path
}

/// Installs a fixture with a fake `uv` that appends its arguments, working directory and
/// `UV_PROJECT_ENVIRONMENT` to a log, and returns that log.
#[cfg(unix)]
fn install_with_recording_uv(project: &PathBuf) -> String {
    use std::os::unix::fs::PermissionsExt;
    let bin = project.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(
        bin.join("uv"),
        "#!/bin/sh\n{ echo \"args=$*\"; echo \"cwd=$(pwd)\"; echo \"env=$UV_PROJECT_ENVIRONMENT\"; } >> \"$UV_LOG\"\nexit 0\n",
    )
    .unwrap();
    fs::set_permissions(bin.join("uv"), fs::Permissions::from_mode(0o755)).unwrap();
    let log = project.join("uv.log");

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["install", project.to_str().unwrap()])
        .env("MCPCTL_HOME", project.join("state"))
        .env("PATH", &bin)
        .env("UV_LOG", &log)
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    fs::read_to_string(log).unwrap()
}

/// Lays out an installed Python package whose entrypoint lives only in `runtime/.venv/bin`,
/// the layout `uv sync` produces, and returns the state home.
#[cfg(unix)]
fn installed_python_package(project: &PathBuf, entrypoint_script: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let state_home = project.join("state");
    let package = state_home.join("packages/example-mcp/1.0.0");
    fs::create_dir_all(package.join("source")).unwrap();
    fs::copy(project.join("mcpctl.toml"), package.join("source/mcpctl.toml")).unwrap();
    let venv_bin = package.join("runtime/.venv/bin");
    fs::create_dir_all(&venv_bin).unwrap();
    fs::write(venv_bin.join("example-mcp"), entrypoint_script).unwrap();
    fs::set_permissions(venv_bin.join("example-mcp"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        state_home.join("registry.json"),
        r#"{"schema_version":1,"packages":{"example-mcp":{"active_version":"1.0.0","versions":[{"version":"1.0.0","runtime":"python","source":"local","installed_at":"now"}]}}}"#,
    )
    .unwrap();
    state_home
}

#[cfg(unix)]
#[test]
fn run_launches_python_entrypoint_from_runtime_venv() {
    let project = temporary_project();
    let state_home = installed_python_package(&project, "#!/bin/sh\nprintf venv-ran\n");

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["run", "example-mcp"])
        .env("MCPCTL_HOME", &state_home)
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "venv-ran");

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn doctor_finds_the_python_interpreter_in_runtime_venv() {
    use std::os::unix::fs::PermissionsExt;
    let project = temporary_project();
    let state_home = installed_python_package(&project, "#!/bin/sh\n");
    let python = state_home.join("packages/example-mcp/1.0.0/runtime/.venv/bin/python");
    fs::write(&python, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&python, fs::Permissions::from_mode(0o755)).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .arg("doctor")
        .env("MCPCTL_HOME", &state_home)
        .output()
        .unwrap();

    assert!(output.status.success(), "stdout was: {}", String::from_utf8_lossy(&output.stdout));
    assert!(String::from_utf8_lossy(&output.stdout).contains("OK"));

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn doctor_flags_a_python_package_with_only_a_legacy_runtime_bin_python() {
    use std::os::unix::fs::PermissionsExt;
    let project = temporary_project();
    let state_home = installed_python_package(&project, "#!/bin/sh\n");
    let package = state_home.join("packages/example-mcp/1.0.0");
    fs::remove_dir_all(package.join("runtime/.venv")).unwrap();
    fs::create_dir_all(package.join("runtime/bin")).unwrap();
    fs::write(package.join("runtime/bin/python"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(package.join("runtime/bin/python"), fs::Permissions::from_mode(0o755)).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .arg("doctor")
        .env("MCPCTL_HOME", &state_home)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("missing entrypoint"));

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn run_puts_the_runtime_venv_bin_first_on_path() {
    let project = temporary_project();
    let state_home = installed_python_package(&project, "#!/bin/sh\nprintf '%s' \"$PATH\"\n");

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["run", "example-mcp"])
        .env("MCPCTL_HOME", &state_home)
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    let path = String::from_utf8_lossy(&output.stdout).into_owned();
    let first = path.split(':').next().unwrap();
    assert!(first.ends_with("packages/example-mcp/1.0.0/runtime/.venv/bin"), "PATH was {path}");

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn failed_uv_sync_fails_the_install_and_leaves_no_state() {
    use std::os::unix::fs::PermissionsExt;
    let project = temporary_project();
    let state_home = project.join("state");
    let bin = project.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("uv"), "#!/bin/sh\n[ \"$1\" = sync ] && { echo 'lockfile is out of date' >&2; exit 1; }\nexit 0\n").unwrap();
    fs::set_permissions(bin.join("uv"), fs::Permissions::from_mode(0o755)).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["install", project.to_str().unwrap()])
        .env("MCPCTL_HOME", &state_home)
        .env("PATH", &bin)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("uv sync"), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    assert!(!state_home.join("packages/example-mcp/1.0.0").exists());
    assert!(!state_home.join("registry.json").exists());

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn install_runs_frozen_uv_sync_from_the_snapshot_into_runtime_venv() {
    let project = temporary_project();

    let log = install_with_recording_uv(&project);

    assert!(log.contains("args=sync --frozen --no-dev"), "uv log was:\n{log}");
    let cwd = log.lines().skip_while(|line| !line.starts_with("args=sync --frozen --no-dev")).nth(1).unwrap();
    assert!(cwd.ends_with("packages/example-mcp/1.0.0/source"), "cwd was {cwd}");
    let env = log.lines().skip_while(|line| !line.starts_with("args=sync --frozen --no-dev")).nth(2).unwrap();
    assert!(env.ends_with("packages/example-mcp/1.0.0/runtime/.venv"), "env was {env}");

    fs::remove_dir_all(project).unwrap();
}

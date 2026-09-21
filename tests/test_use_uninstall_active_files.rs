use std::{fs, path::PathBuf, process::{Command, Output}, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_project() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-active-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    write_python_manifest(&path, "1.0.0");
    fs::write(path.join("pyproject.toml"), "[project]\nname = \"example-mcp\"\nversion = \"1.0.0\"\n").unwrap();
    fs::write(path.join("uv.lock"), "version = 1\n").unwrap();
    path
}

fn write_python_manifest(project: &PathBuf, version: &str) {
    fs::write(
        project.join("mcpctl.toml"),
        format!("name = \"example-mcp\"\nversion = \"{version}\"\n[runtime]\ntype = \"python\"\npython = \">=3.12\"\n[install]\nstrategy = \"uv\"\nentrypoint = \"example-mcp\"\n"),
    )
    .unwrap();
}

/// Installs the fixture with a no-op fake `uv`; returns the process output and the state home.
#[cfg(unix)]
fn install(project: &PathBuf) -> (Output, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let bin = project.join("fake-bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("uv"), "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(bin.join("uv"), fs::Permissions::from_mode(0o755)).unwrap();
    let state_home = project.join("state");
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["install", project.to_str().unwrap()])
        .env("MCPCTL_HOME", &state_home)
        .env("PATH", &bin)
        .output()
        .unwrap();
    (output, state_home)
}

fn mcpctl(state_home: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(args).env("MCPCTL_HOME", state_home).output().unwrap()
}

fn read_json(path: PathBuf) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(&path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))).unwrap()
}

#[cfg(unix)]
#[test]
fn use_rewrites_the_active_file_to_the_selected_version() {
    let project = temporary_project();
    let (first, state_home) = install(&project);
    assert!(first.status.success());
    write_python_manifest(&project, "2.0.0");
    assert!(install(&project).0.status.success());

    let output = mcpctl(&state_home, &["use", "example-mcp@2.0.0"]);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    let active = read_json(state_home.join("active/example-mcp.json"));
    assert_eq!(active["version"], "2.0.0");
    assert_eq!(active["schema_version"], 1);
    // The atomic write must not leave temp files behind in active/.
    let leftovers: Vec<_> = fs::read_dir(state_home.join("active")).unwrap().map(|entry| entry.unwrap().file_name()).collect();
    assert_eq!(leftovers, vec![std::ffi::OsString::from("example-mcp.json")]);

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn uninstalling_the_last_version_removes_the_active_file() {
    let project = temporary_project();
    let (first, state_home) = install(&project);
    assert!(first.status.success());

    let output = mcpctl(&state_home, &["uninstall", "example-mcp@1.0.0"]);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    assert!(!state_home.join("active/example-mcp.json").exists());
    assert!(!state_home.join("packages/example-mcp/1.0.0").exists());

    fs::remove_dir_all(project).unwrap();
}

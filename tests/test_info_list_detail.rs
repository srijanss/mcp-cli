use std::{fs, path::PathBuf, process::{Command, Output}, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_project() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-info-test-{}-{}",
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

#[cfg(unix)]
#[test]
fn info_accepts_name_at_version_and_prints_detail() {
    let project = temporary_project();
    let (first, state_home) = install(&project);
    assert!(first.status.success());
    write_python_manifest(&project, "2.0.0");
    assert!(install(&project).0.status.success());

    let output = mcpctl(&state_home, &["info", "example-mcp@2.0.0"]);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8_lossy(&output.stdout);
    let canonical_project = fs::canonicalize(&project).unwrap();
    assert!(text.contains("example-mcp"), "{text}");
    assert!(text.contains("2.0.0"), "{text}");
    assert!(!text.contains("1.0.0"), "only the selected version is shown: {text}");
    assert!(text.contains(canonical_project.to_str().unwrap()), "source path missing: {text}");
    assert!(text.contains(state_home.join("packages/example-mcp/2.0.0").to_str().unwrap()), "install path missing: {text}");
    assert!(text.contains("entrypoint: example-mcp"), "{text}");
    assert!(text.contains("python: >=3.12"), "{text}");
    assert!(text.contains("active: no"), "{text}");
    assert!(text.contains("installed_at: 20"), "expected an RFC 3339 timestamp: {text}");

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn info_rejects_a_version_that_is_not_installed() {
    let project = temporary_project();
    let (first, state_home) = install(&project);
    assert!(first.status.success());

    let output = mcpctl(&state_home, &["info", "example-mcp@9.9.9"]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not installed"));

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn list_prints_a_table_with_a_header_row() {
    let project = temporary_project();
    let (first, state_home) = install(&project);
    assert!(first.status.success());

    let output = mcpctl(&state_home, &["list"]);

    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines();
    let header = lines.next().unwrap();
    assert_eq!(header.split_whitespace().collect::<Vec<_>>(), ["NAME", "ACTIVE", "VERSIONS", "RUNTIME"]);
    let row = lines.next().unwrap();
    assert_eq!(row.split_whitespace().collect::<Vec<_>>(), ["example-mcp", "1.0.0", "1.0.0", "python"]);
    // Columns are aligned: the row's second column starts where the header's does.
    assert_eq!(header.find("ACTIVE"), row.find("1.0.0"));

    fs::remove_dir_all(project).unwrap();
}

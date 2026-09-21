use std::{fs, path::PathBuf, process::{Command, Output}, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_project() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-metadata-test-{}-{}",
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

fn read_json(path: PathBuf) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(&path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))).unwrap()
}

#[cfg(unix)]
#[test]
fn python_metadata_records_manifest_and_lockfile_hashes() {
    let project = temporary_project();

    let (output, state_home) = install(&project);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    let metadata = read_json(state_home.join("packages/example-mcp/1.0.0/metadata.json"));
    // SHA-256 of the fixture's uv.lock ("version = 1\n"), computed independently with `shasum -a 256`.
    assert_eq!(metadata["lockfile_sha256"], "dbab12665d98aef021ba64953c61b0ed8a908cfb56a1c01e2fcb4b052b71a2a1");
    let manifest_hash = metadata["manifest_sha256"].as_str().unwrap();
    assert_eq!(manifest_hash.len(), 64);
    assert!(manifest_hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_ne!(manifest_hash, metadata["lockfile_sha256"].as_str().unwrap());

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn python_metadata_records_the_uv_version() {
    use std::os::unix::fs::PermissionsExt;
    let project = temporary_project();
    let bin = project.join("fake-bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("uv"), "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'uv 9.9.9 (abc123 2026-01-01)'; fi\nexit 0\n").unwrap();
    fs::set_permissions(bin.join("uv"), fs::Permissions::from_mode(0o755)).unwrap();
    let state_home = project.join("state");

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["install", project.to_str().unwrap()])
        .env("MCPCTL_HOME", &state_home)
        .env("PATH", &bin)
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(read_json(state_home.join("packages/example-mcp/1.0.0/metadata.json"))["uv_version"], "9.9.9");

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn binary_metadata_records_the_executable_checksum() {
    use std::os::unix::fs::PermissionsExt;
    let project = temporary_project();
    fs::write(project.join("mcpctl.toml"), "name = \"binary-mcp\"\nversion = \"2.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"dist/binary-mcp\"\n").unwrap();
    fs::create_dir_all(project.join("dist")).unwrap();
    fs::write(project.join("dist/binary-mcp"), "abc").unwrap();
    fs::set_permissions(project.join("dist/binary-mcp"), fs::Permissions::from_mode(0o755)).unwrap();
    let state_home = project.join("state");

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["install", project.to_str().unwrap()])
        .env("MCPCTL_HOME", &state_home)
        .env("PATH", "")
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    let metadata = read_json(state_home.join("packages/binary-mcp/2.0.0/metadata.json"));
    // SHA-256 of "abc" is a standard test vector.
    assert_eq!(metadata["executable_sha256"], "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert!(metadata.get("uv_version").is_none(), "uv_version is Python only");

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn source_tree_hash_covers_the_installed_snapshot_not_excluded_files() {
    let project = temporary_project();
    let (first, first_home) = install(&project);
    assert!(first.status.success());
    let first_hash = read_json(first_home.join("packages/example-mcp/1.0.0/metadata.json"))["source_tree_sha256"].as_str().unwrap().to_owned();
    assert_eq!(first_hash.len(), 64);

    // Files the snapshot excludes must not change the hash (a secret .env must never influence it).
    fs::write(project.join(".env"), "SECRET=1\n").unwrap();
    fs::remove_dir_all(project.join("state")).unwrap();
    let (again, again_home) = install(&project);
    assert!(again.status.success());
    let again_hash = read_json(again_home.join("packages/example-mcp/1.0.0/metadata.json"))["source_tree_sha256"].as_str().unwrap().to_owned();
    assert_eq!(first_hash, again_hash);

    // A change to an included file must change it.
    fs::write(project.join("uv.lock"), "version = 2\n").unwrap();
    fs::remove_dir_all(project.join("state")).unwrap();
    let (changed, changed_home) = install(&project);
    assert!(changed.status.success());
    let changed_hash = read_json(changed_home.join("packages/example-mcp/1.0.0/metadata.json"))["source_tree_sha256"].as_str().unwrap().to_owned();
    assert_ne!(first_hash, changed_hash);

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn binary_metadata_has_no_lockfile_hash() {
    use std::os::unix::fs::PermissionsExt;
    let project = temporary_project();
    fs::write(project.join("mcpctl.toml"), "name = \"binary-mcp\"\nversion = \"2.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"binary-mcp\"\n").unwrap();
    fs::write(project.join("binary-mcp"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(project.join("binary-mcp"), fs::Permissions::from_mode(0o755)).unwrap();
    let state_home = project.join("state");

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["install", project.to_str().unwrap()])
        .env("MCPCTL_HOME", &state_home)
        .env("PATH", "")
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    let metadata = read_json(state_home.join("packages/binary-mcp/2.0.0/metadata.json"));
    assert!(metadata.get("lockfile_sha256").is_none(), "lockfile_sha256 is Python only");
    assert!(metadata["manifest_sha256"].as_str().is_some());

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn first_install_of_a_name_writes_the_active_selection_file() {
    let project = temporary_project();

    let (output, state_home) = install(&project);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    let active = read_json(state_home.join("active/example-mcp.json"));
    assert_eq!(active["schema_version"], 1);
    assert_eq!(active["name"], "example-mcp");
    assert_eq!(active["version"], "1.0.0");

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn installing_a_later_version_leaves_the_active_selection_unchanged() {
    let project = temporary_project();
    let (first, state_home) = install(&project);
    assert!(first.status.success());
    write_python_manifest(&project, "2.0.0");

    let (second, _) = install(&project);

    assert!(second.status.success(), "stderr was: {}", String::from_utf8_lossy(&second.stderr));
    assert!(state_home.join("packages/example-mcp/2.0.0/metadata.json").is_file());
    assert_eq!(read_json(state_home.join("active/example-mcp.json"))["version"], "1.0.0");

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn binary_install_writes_metadata_with_binary_runtime_type() {
    use std::os::unix::fs::PermissionsExt;
    let project = temporary_project();
    fs::write(project.join("mcpctl.toml"), "name = \"binary-mcp\"\nversion = \"2.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"dist/binary-mcp\"\n").unwrap();
    fs::create_dir_all(project.join("dist")).unwrap();
    fs::write(project.join("dist/binary-mcp"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(project.join("dist/binary-mcp"), fs::Permissions::from_mode(0o755)).unwrap();
    let state_home = project.join("state");

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["install", project.to_str().unwrap()])
        .env("MCPCTL_HOME", &state_home)
        .env("PATH", "")
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    let metadata = read_json(state_home.join("packages/binary-mcp/2.0.0/metadata.json"));
    assert_eq!(metadata["runtime_type"], "binary");
    assert_eq!(metadata["name"], "binary-mcp");
    assert_eq!(metadata["entrypoint_relative_path"], "dist/binary-mcp");

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn install_writes_per_version_metadata() {
    let project = temporary_project();

    let (output, state_home) = install(&project);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    let metadata = read_json(state_home.join("packages/example-mcp/1.0.0/metadata.json"));
    assert_eq!(metadata["schema_version"], 1);
    assert_eq!(metadata["name"], "example-mcp");
    assert_eq!(metadata["version"], "1.0.0");
    assert_eq!(metadata["runtime_type"], "python");
    assert_eq!(metadata["mcpctl_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(metadata["entrypoint_relative_path"], "example-mcp");
    assert_eq!(metadata["source_path"], project.canonicalize().unwrap().to_str().unwrap());
    let installed_at = metadata["installed_at"].as_str().unwrap();
    assert!(
        installed_at.len() == 20 && installed_at.ends_with('Z') && installed_at.as_bytes()[10] == b'T',
        "installed_at should be RFC 3339 UTC, got {installed_at}"
    );

    fs::remove_dir_all(project).unwrap();
}

use std::{fs, path::PathBuf, process::{Command, Output}, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_project(mode: u32) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!("mcpctl-binary-cleanup-test-{}-{}", std::process::id(), NEXT_ID.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("mcpctl.toml"), "name = \"bin-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"bin-mcp\"\n").unwrap();
    fs::write(path.join("bin-mcp"), "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(path.join("bin-mcp"), fs::Permissions::from_mode(mode)).unwrap();
    path
}

fn install(project: &PathBuf) -> (Output, PathBuf) {
    let state_home = project.join("state");
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["install", project.to_str().unwrap()]).env("MCPCTL_HOME", &state_home).output().unwrap();
    (output, state_home)
}

#[cfg(unix)]
#[test]
fn binary_metadata_records_the_platform_and_executable_checksum() {
    let project = temporary_project(0o755);

    let (output, state_home) = install(&project);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    let metadata: serde_json::Value = serde_json::from_str(&fs::read_to_string(state_home.join("packages/bin-mcp/1.0.0/metadata.json")).unwrap()).unwrap();
    assert_eq!(metadata["platform"], format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH));
    // SHA-256 of "#!/bin/sh\nexit 0\n", computed independently with `shasum -a 256`.
    assert_eq!(metadata["executable_sha256"], "306c6ca7407560340797866e077e053627ad409277d1b9da58106fce4cf717cb");

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn non_executable_binary_is_rejected_with_one_clear_message_and_no_leftovers() {
    let project = temporary_project(0o644);

    let (output, state_home) = install(&project);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr.trim(), "binary entrypoint is not executable");
    assert!(!state_home.join("packages/bin-mcp").exists());

    fs::remove_dir_all(project).unwrap();
}

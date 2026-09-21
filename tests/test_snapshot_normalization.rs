use std::{fs, path::PathBuf, process::{Command, Output}, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_project() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-snapshot-test-{}-{}",
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

#[cfg(unix)]
#[test]
fn snapshot_preserves_relative_symlinks_that_stay_inside_the_source_root() {
    let project = temporary_project();
    fs::create_dir_all(project.join("pkg")).unwrap();
    fs::write(project.join("pkg/real.py"), "x = 1\n").unwrap();
    std::os::unix::fs::symlink("pkg/real.py", project.join("alias.py")).unwrap();

    let (output, state_home) = install(&project);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    let alias = state_home.join("packages/example-mcp/1.0.0/source/alias.py");
    assert_eq!(fs::read_link(&alias).unwrap(), PathBuf::from("pkg/real.py"));
    assert_eq!(fs::read_to_string(&alias).unwrap(), "x = 1\n");

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn install_rejects_a_symlink_that_escapes_the_source_root() {
    let project = temporary_project();
    let outside = std::env::temp_dir().join(format!("mcpctl-snapshot-outside-{}", std::process::id()));
    fs::write(&outside, "outside the source root").unwrap();
    std::os::unix::fs::symlink(&outside, project.join("escape.txt")).unwrap();

    let (output, state_home) = install(&project);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("escape.txt") && stderr.contains("symlink"), "stderr was: {stderr}");
    assert!(!state_home.join("packages/example-mcp/1.0.0").exists());
    assert!(!state_home.join("registry.json").exists());

    fs::remove_file(outside).unwrap();
    fs::remove_dir_all(project).unwrap();
}

// The next two tests characterize branches of the symlink check; they were written after the code.
#[cfg(unix)]
#[test]
fn install_rejects_a_relative_symlink_that_climbs_out_of_the_source_root() {
    let project = temporary_project();
    let parent = project.parent().unwrap().join(format!("mcpctl-snapshot-sibling-{}", std::process::id()));
    fs::write(&parent, "sibling of the source root").unwrap();
    let relative = format!("../{}", parent.file_name().unwrap().to_str().unwrap());
    std::os::unix::fs::symlink(&relative, project.join("climb.txt")).unwrap();

    let (output, state_home) = install(&project);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("escapes the source root"), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    assert!(!state_home.join("packages/example-mcp/1.0.0").exists());

    fs::remove_file(parent).unwrap();
    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn install_rejects_a_dangling_symlink() {
    let project = temporary_project();
    std::os::unix::fs::symlink("does-not-exist", project.join("dangling")).unwrap();

    let (output, state_home) = install(&project);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("does not resolve"), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    assert!(!state_home.join("packages/example-mcp/1.0.0").exists());

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn snapshot_excludes_python_bytecode_and_build_artifacts_but_keeps_sources() {
    let project = temporary_project();
    fs::create_dir_all(project.join("pkg")).unwrap();
    fs::write(project.join("pkg/module.py"), "x = 1\n").unwrap();
    fs::write(project.join("pkg/module.pyc"), "bytecode").unwrap();
    fs::create_dir_all(project.join("example_mcp.egg-info")).unwrap();
    fs::write(project.join("example_mcp.egg-info/PKG-INFO"), "meta").unwrap();
    fs::create_dir_all(project.join("build/lib")).unwrap();
    fs::write(project.join("build/lib/module.py"), "x = 1\n").unwrap();
    fs::create_dir_all(project.join("dist")).unwrap();
    fs::write(project.join("dist/keep-me"), "binary entrypoints may live in dist/").unwrap();

    let (output, state_home) = install(&project);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    let source = state_home.join("packages/example-mcp/1.0.0/source");
    assert!(source.join("pkg/module.py").is_file());
    assert!(!source.join("pkg/module.pyc").exists());
    assert!(!source.join("example_mcp.egg-info").exists());
    assert!(!source.join("build").exists());
    assert!(source.join("dist/keep-me").is_file(), "dist/ is deliberately not excluded");

    fs::remove_dir_all(project).unwrap();
}

#[cfg(unix)]
#[test]
fn snapshot_excludes_env_files_but_keeps_env_templates() {
    let project = temporary_project();
    fs::write(project.join(".env"), "SECRET=1\n").unwrap();
    fs::write(project.join(".env.local"), "SECRET=2\n").unwrap();
    fs::write(project.join(".env.example"), "SECRET=\n").unwrap();

    let (output, state_home) = install(&project);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    let source = state_home.join("packages/example-mcp/1.0.0/source");
    assert!(!source.join(".env").exists(), ".env must not be persisted");
    assert!(!source.join(".env.local").exists(), ".env.local must not be persisted");
    assert!(source.join(".env.example").is_file(), "templates may be included");
    assert!(source.join("mcpctl.toml").is_file());

    fs::remove_dir_all(project).unwrap();
}

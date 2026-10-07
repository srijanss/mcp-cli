use std::{fs, path::{Path, PathBuf}, process::{Command, Output}, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_dir() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-catalog-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

/// Writes a binary-runtime MCP source at `workspace/dir` with a one-file scaffold.
fn write_package(workspace: &Path, dir: &str, name: &str, version: &str) -> PathBuf {
    let source = workspace.join(dir);
    fs::create_dir_all(source.join("bin")).unwrap();
    fs::create_dir_all(source.join("templates")).unwrap();
    fs::write(
        source.join("mcpctl.toml"),
        format!(
            "name = \"{name}\"\nversion = \"{version}\"\ndescription = \"The {name} server\"\n\n[runtime]\ntype = \"binary\"\n\n\
             [install]\nentrypoint = \"bin/{name}\"\n\n[scaffold]\n\n[[scaffold.files]]\nfrom = \"templates/{name}.md\"\nto = \"docs/{name}.md\"\n"
        ),
    )
    .unwrap();
    fs::write(source.join("templates").join(format!("{name}.md")), format!("{name}@{version}")).unwrap();
    fs::write(source.join("bin").join(name), "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(source.join("bin").join(name), fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::canonicalize(source).unwrap()
}

/// Runs mcpctl in `directory` with its own config home and state home under `workspace`.
fn mcpctl(workspace: &Path, directory: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(arguments)
        .current_dir(directory)
        .env("MCPCTL_CONFIG_HOME", workspace.join("config"))
        .env("MCPCTL_HOME", workspace.join("state"))
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn catalog_add_records_the_absolute_source_in_the_config_home_without_installing_it() {
    let workspace = temporary_dir();
    let source = write_package(&workspace, "project-mcp", "project-mcp", "0.4.3");
    let project = workspace.join("checkout-service");
    fs::create_dir_all(&project).unwrap();

    let output = mcpctl(&workspace, &project, &["catalog", "add", "../project-mcp"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), format!("Added project-mcp@0.4.3 ({})\n", source.display()));
    let catalog = fs::read_to_string(workspace.join("config/catalog.toml")).unwrap();
    assert_eq!(catalog, format!("[[mcp]]\nsource = {:?}\n", source.display().to_string()));
    assert!(!workspace.join("state/registry.json").exists(), "catalog add must not install");
    assert!(!workspace.join("state/packages").exists(), "catalog add must not install");
}

#[test]
fn catalog_add_appends_after_the_sources_already_in_the_catalog() {
    let workspace = temporary_dir();
    let project_mcp = write_package(&workspace, "project-mcp", "project-mcp", "0.4.3");
    let design_advisor = write_package(&workspace, "design-advisor-mcp", "design-advisor-mcp", "0.2.0");

    assert!(mcpctl(&workspace, &workspace, &["catalog", "add", "project-mcp"]).status.success());
    let output = mcpctl(&workspace, &workspace, &["catalog", "add", "design-advisor-mcp"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(
        fs::read_to_string(workspace.join("config/catalog.toml")).unwrap(),
        format!(
            "[[mcp]]\nsource = {:?}\n\n[[mcp]]\nsource = {:?}\n",
            project_mcp.display().to_string(),
            design_advisor.display().to_string()
        )
    );
}

#[test]
fn catalog_add_fails_naming_a_source_that_is_missing_or_not_a_valid_package_and_leaves_the_catalog_unchanged() {
    let workspace = temporary_dir();
    write_package(&workspace, "project-mcp", "project-mcp", "0.4.3");
    assert!(mcpctl(&workspace, &workspace, &["catalog", "add", "project-mcp"]).status.success());
    let catalog_before = fs::read_to_string(workspace.join("config/catalog.toml")).unwrap();
    fs::create_dir_all(workspace.join("not-a-package")).unwrap();
    let broken = write_package(&workspace, "broken-mcp", "broken-mcp", "0.1.0");
    fs::write(broken.join("mcpctl.toml"), "name = \"broken-mcp\"\nversion = \"not-semver\"\n").unwrap();

    for (path, reason) in [
        ("missing-mcp", "does not exist"),
        ("not-a-package", "has no mcpctl.toml"),
        ("broken-mcp", "invalid mcpctl.toml"),
    ] {
        let output = mcpctl(&workspace, &workspace, &["catalog", "add", path]);

        assert!(!output.status.success(), "adding {path} should fail");
        let error = stderr(&output);
        assert!(error.contains(&format!("cannot add {path} to the catalog")), "{path}: {error}");
        assert!(error.contains(reason), "{path}: expected {reason:?} in {error}");
        assert_eq!(fs::read_to_string(workspace.join("config/catalog.toml")).unwrap(), catalog_before);
    }
}

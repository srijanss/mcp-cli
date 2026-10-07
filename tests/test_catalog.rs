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

#[test]
fn catalog_add_of_a_source_already_in_the_catalog_changes_nothing() {
    let workspace = temporary_dir();
    let source = write_package(&workspace, "project-mcp", "project-mcp", "0.4.3");
    assert!(mcpctl(&workspace, &workspace, &["catalog", "add", "project-mcp"]).status.success());

    let output = mcpctl(&workspace, &workspace.join("project-mcp"), &["catalog", "add", "."]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "project-mcp is already in the catalog\n");
    assert_eq!(
        fs::read_to_string(workspace.join("config/catalog.toml")).unwrap(),
        format!("[[mcp]]\nsource = {:?}\n", source.display().to_string())
    );
}

#[test]
fn catalog_add_fails_naming_both_sources_when_another_source_already_provides_the_name() {
    let workspace = temporary_dir();
    let original = write_package(&workspace, "project-mcp", "project-mcp", "0.4.3");
    let fork = write_package(&workspace, "project-mcp-fork", "project-mcp", "0.5.0");
    assert!(mcpctl(&workspace, &workspace, &["catalog", "add", "project-mcp"]).status.success());
    let catalog_before = fs::read_to_string(workspace.join("config/catalog.toml")).unwrap();

    let output = mcpctl(&workspace, &workspace, &["catalog", "add", "project-mcp-fork"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains(&format!(
            "project-mcp is already in the catalog from {}; cannot also add {}",
            original.display(),
            fork.display()
        )),
        "stderr: {}",
        stderr(&output)
    );
    assert_eq!(fs::read_to_string(workspace.join("config/catalog.toml")).unwrap(), catalog_before);
}

#[test]
fn catalog_list_shows_each_mcps_metadata_sorted_by_name() {
    let workspace = temporary_dir();
    let project_mcp = write_package(&workspace, "project-mcp", "project-mcp", "0.4.3");
    let design_advisor = write_package(&workspace, "design-advisor-mcp", "design-advisor-mcp", "0.2.0");
    fs::write(
        design_advisor.join("mcpctl.toml"),
        "name = \"design-advisor-mcp\"\nversion = \"0.2.0\"\n\n[runtime]\ntype = \"binary\"\n\n[install]\nentrypoint = \"bin/design-advisor-mcp\"\n",
    )
    .unwrap();
    assert!(mcpctl(&workspace, &workspace, &["catalog", "add", "project-mcp"]).status.success());
    assert!(mcpctl(&workspace, &workspace, &["catalog", "add", "design-advisor-mcp"]).status.success());

    let output = mcpctl(&workspace, &workspace, &["catalog", "list"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        format!(
            "design-advisor-mcp@0.2.0 (binary, no scaffold)\n  source: {}\n\
             project-mcp@0.4.3 (binary, scaffold)\n  The project-mcp server\n  source: {}\n",
            design_advisor.display(),
            project_mcp.display()
        )
    );
}

#[test]
fn catalog_list_of_an_empty_catalog_suggests_catalog_add() {
    let workspace = temporary_dir();

    let output = mcpctl(&workspace, &workspace, &["catalog", "list"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "Catalog is empty\nAdd an MCP with: mcpctl catalog add <path>\n");
}

#[test]
fn catalog_list_reports_sources_that_became_unusable_and_still_lists_the_rest() {
    let workspace = temporary_dir();
    let project_mcp = write_package(&workspace, "project-mcp", "project-mcp", "0.4.3");
    let removed = write_package(&workspace, "removed-mcp", "removed-mcp", "1.0.0");
    let broken = write_package(&workspace, "broken-mcp", "broken-mcp", "0.1.0");
    for path in ["removed-mcp", "project-mcp", "broken-mcp"] {
        assert!(mcpctl(&workspace, &workspace, &["catalog", "add", path]).status.success());
    }
    fs::remove_dir_all(&removed).unwrap();
    fs::write(broken.join("mcpctl.toml"), "name = \"broken-mcp\"\nversion = \"not-semver\"\n").unwrap();

    let output = mcpctl(&workspace, &workspace, &["catalog", "list"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let listing = stdout(&output);
    assert!(
        listing.starts_with(&format!(
            "project-mcp@0.4.3 (binary, scaffold)\n  The project-mcp server\n  source: {}\n",
            project_mcp.display()
        )),
        "{listing}"
    );
    assert!(
        listing.contains(&format!("unavailable: {} does not exist or is not a directory\n", removed.display())),
        "{listing}"
    );
    assert!(listing.contains(&format!("unavailable: invalid mcpctl.toml in {}", broken.display())), "{listing}");
}

#[test]
fn catalog_list_keeps_the_first_source_when_a_hand_edited_catalog_repeats_a_name() {
    let workspace = temporary_dir();
    let original = write_package(&workspace, "project-mcp", "project-mcp", "0.4.3");
    let fork = write_package(&workspace, "project-mcp-fork", "project-mcp", "0.5.0");
    fs::create_dir_all(workspace.join("config")).unwrap();
    fs::write(
        workspace.join("config/catalog.toml"),
        format!("[[mcp]]\nsource = {:?}\n\n[[mcp]]\nsource = {:?}\n", original.display().to_string(), fork.display().to_string()),
    )
    .unwrap();

    let output = mcpctl(&workspace, &workspace, &["catalog", "list"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        format!(
            "project-mcp@0.4.3 (binary, scaffold)\n  The project-mcp server\n  source: {}\n\
             WARNING: project-mcp is provided by both {} and {}; using {}\n",
            original.display(),
            original.display(),
            fork.display(),
            original.display()
        )
    );
}

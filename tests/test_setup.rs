use std::{fs, path::{Path, PathBuf}, process::{Command, Output}, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_dir() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-setup-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    fs::canonicalize(path).unwrap()
}

/// Writes a binary-runtime MCP source at `workspace/dir` whose entrypoint prints `name@version`, with a one-file scaffold.
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
    fs::write(source.join("bin").join(name), format!("#!/bin/sh\nprintf '{name}@{version}'\n")).unwrap();
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

/// A workspace whose catalog holds `project-mcp` (0.4.3) and `design-advisor-mcp` (0.2.0), plus an empty `checkout-service/`.
fn workspace_with_catalog() -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let workspace = temporary_dir();
    let project_mcp = write_package(&workspace, "project-mcp", "project-mcp", "0.4.3");
    let design_advisor = write_package(&workspace, "design-advisor-mcp", "design-advisor-mcp", "0.2.0");
    assert!(mcpctl(&workspace, &workspace, &["catalog", "add", "project-mcp"]).status.success());
    assert!(mcpctl(&workspace, &workspace, &["catalog", "add", "design-advisor-mcp"]).status.success());
    let project = workspace.join("checkout-service");
    fs::create_dir_all(&project).unwrap();
    (workspace, project, project_mcp, design_advisor)
}

#[test]
fn setup_all_creates_the_project_manifest_from_the_catalog_then_locks_and_installs_it() {
    let (workspace, project, project_mcp, design_advisor) = workspace_with_catalog();

    let output = mcpctl(&workspace, &project, &["setup", "--all"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(
        stdout(&output).starts_with(
            "Selected MCPs\n  design-advisor-mcp\n  project-mcp\nCreating .mcpctl.toml\nResolving versions\nInstalling required MCPs\nCreating .mcpctl.lock\n"
        ),
        "stdout: {}",
        stdout(&output)
    );
    assert_eq!(
        fs::read_to_string(project.join(".mcpctl.toml")).unwrap(),
        format!(
            "[project]\nname = \"checkout-service\"\n\n\
             [[mcp]]\nname = \"design-advisor-mcp\"\nversion = \"^0.2.0\"\nsource = {:?}\n\n\
             [[mcp]]\nname = \"project-mcp\"\nversion = \"^0.4.3\"\nsource = {:?}\n",
            design_advisor.display().to_string(),
            project_mcp.display().to_string()
        )
    );
    let lock = mcp_cli::project_lock::parse_project_lock(&fs::read_to_string(project.join(".mcpctl.lock")).unwrap()).unwrap();
    let locked: Vec<_> = lock.mcp.iter().map(|mcp| (mcp.name.as_str(), mcp.version.as_str())).collect();
    assert_eq!(locked, [("design-advisor-mcp", "0.2.0"), ("project-mcp", "0.4.3")]);
    assert_eq!(mcpctl(&workspace, &project, &["run", "project-mcp"]).stdout, b"project-mcp@0.4.3");
    assert_eq!(mcpctl(&workspace, &project, &["run", "design-advisor-mcp"]).stdout, b"design-advisor-mcp@0.2.0");
}

#[test]
fn setup_with_an_empty_catalog_fails_suggesting_catalog_add_and_writes_nothing() {
    let workspace = temporary_dir();
    let project = workspace.join("checkout-service");
    fs::create_dir_all(&project).unwrap();

    let output = mcpctl(&workspace, &project, &["setup", "--all"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("Catalog is empty; add an MCP with: mcpctl catalog add <path>"), "stderr: {}", stderr(&output));
    assert_eq!(fs::read_dir(&project).unwrap().count(), 0);
}

#[test]
fn setup_applies_every_selected_scaffold_then_checks_requirements() {
    let (workspace, project, project_mcp, _) = workspace_with_catalog();
    let manifest = fs::read_to_string(project_mcp.join("mcpctl.toml")).unwrap();
    fs::write(project_mcp.join("mcpctl.toml"), manifest.replace("[scaffold]\n", "[scaffold]\nrequires = [\"mcpctl-fake-tool\"]\n")).unwrap();

    let output = mcpctl(&workspace, &project, &["setup", "--all"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "Selected MCPs\n  design-advisor-mcp\n  project-mcp\nCreating .mcpctl.toml\nResolving versions\nInstalling required MCPs\nCreating .mcpctl.lock\n\
         Applying scaffolds\nInitializing design-advisor-mcp@0.2.0\nCopied docs/design-advisor-mcp.md\nInitializing project-mcp@0.4.3\nCopied docs/project-mcp.md\n\
         Checking requirements\nWARNING: mcpctl-fake-tool is required by project-mcp but was not found on PATH\n\
         Project MCP environment ready.\n"
    );
    assert_eq!(fs::read_to_string(project.join("docs/project-mcp.md")).unwrap(), "project-mcp@0.4.3");
    assert_eq!(fs::read_to_string(project.join("docs/design-advisor-mcp.md")).unwrap(), "design-advisor-mcp@0.2.0");
}

#[test]
fn setup_fails_naming_each_mcp_whose_scaffold_cannot_be_applied() {
    let (workspace, project, _, _) = workspace_with_catalog();
    // A `docs` file where the scaffolds expect a directory makes every copy into `docs/` fail.
    fs::write(project.join("docs"), "not a directory").unwrap();

    let output = mcpctl(&workspace, &project, &["setup", "--all"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("setup failed for design-advisor-mcp, project-mcp"), "stderr: {}", stderr(&output));
    assert!(!stdout(&output).contains("Project MCP environment ready."), "stdout: {}", stdout(&output));
}

#[test]
fn setup_mcp_selects_only_the_named_catalog_mcps() {
    let (workspace, project, project_mcp, _) = workspace_with_catalog();

    let output = mcpctl(&workspace, &project, &["setup", "--mcp", "project-mcp"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).starts_with("Selected MCPs\n  project-mcp\nCreating .mcpctl.toml\n"), "stdout: {}", stdout(&output));
    assert_eq!(
        fs::read_to_string(project.join(".mcpctl.toml")).unwrap(),
        format!(
            "[project]\nname = \"checkout-service\"\n\n[[mcp]]\nname = \"project-mcp\"\nversion = \"^0.4.3\"\nsource = {:?}\n",
            project_mcp.display().to_string()
        )
    );
    assert!(!project.join("docs/design-advisor-mcp.md").exists());
}

#[test]
fn setup_mcp_of_a_name_not_in_the_catalog_fails_listing_the_available_names_and_writes_nothing() {
    let (workspace, project, _, _) = workspace_with_catalog();

    let output = mcpctl(&workspace, &project, &["setup", "--mcp", "project-mcp", "--mcp", "missing-mcp"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("missing-mcp is not an available catalog MCP; available: design-advisor-mcp, project-mcp"),
        "stderr: {}",
        stderr(&output)
    );
    assert_eq!(stdout(&output), "");
    assert_eq!(fs::read_dir(&project).unwrap().count(), 0);
}

#[test]
fn setup_without_a_selection_off_a_terminal_fails_suggesting_all_or_mcp() {
    let (workspace, project, _, _) = workspace_with_catalog();

    let output = mcpctl(&workspace, &project, &["setup"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("setup needs a terminal to pick MCPs; pass --all or --mcp <name>"),
        "stderr: {}",
        stderr(&output)
    );
    assert_eq!(fs::read_dir(&project).unwrap().count(), 0);
}

#[test]
fn setup_all_skips_unavailable_catalog_entries_with_a_warning_and_sets_up_the_rest() {
    let (workspace, project, _, design_advisor) = workspace_with_catalog();
    fs::remove_dir_all(&design_advisor).unwrap();

    let output = mcpctl(&workspace, &project, &["setup", "--all"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(
        stdout(&output).starts_with(&format!(
            "WARNING: skipping unavailable catalog entry {}: {} does not exist or is not a directory\nSelected MCPs\n  project-mcp\n",
            design_advisor.display(),
            design_advisor.display()
        )),
        "stdout: {}",
        stdout(&output)
    );
    assert!(!fs::read_to_string(project.join(".mcpctl.toml")).unwrap().contains("design-advisor-mcp"));
}

#[test]
fn setup_keeps_the_first_source_when_a_hand_edited_catalog_repeats_a_name() {
    let (workspace, project, project_mcp, _) = workspace_with_catalog();
    let fork = write_package(&workspace, "project-mcp-fork", "project-mcp", "9.9.9");
    let catalog = fs::read_to_string(workspace.join("config/catalog.toml")).unwrap();
    fs::write(workspace.join("config/catalog.toml"), format!("{catalog}\n[[mcp]]\nsource = {:?}\n", fork.display().to_string())).unwrap();

    let output = mcpctl(&workspace, &project, &["setup", "--mcp", "project-mcp"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(
        stdout(&output).contains(&format!(
            "WARNING: project-mcp is provided by both {} and {}; using {}\n",
            project_mcp.display(),
            fork.display(),
            project_mcp.display()
        )),
        "stdout: {}",
        stdout(&output)
    );
    let manifest = fs::read_to_string(project.join(".mcpctl.toml")).unwrap();
    assert!(manifest.contains(&format!("source = {:?}", project_mcp.display().to_string())), "{manifest}");
    assert!(!manifest.contains("9.9.9"), "{manifest}");
}

#[test]
fn setup_rerun_keeps_the_existing_manifest_as_written_and_appends_only_new_mcps() {
    let (workspace, project, _, design_advisor) = workspace_with_catalog();
    let written = "# hand-written\n[project]\nname = \"custom-name\"\n\n[[mcp]]\nname = \"project-mcp\"\nversion = \"^0.4\"\nsource = \"../project-mcp\"\n";
    fs::write(project.join(".mcpctl.toml"), written).unwrap();

    let output = mcpctl(&workspace, &project, &["setup", "--mcp", "project-mcp", "--mcp", "design-advisor-mcp"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(
        fs::read_to_string(project.join(".mcpctl.toml")).unwrap(),
        format!(
            "{written}\n[[mcp]]\nname = \"design-advisor-mcp\"\nversion = \"^0.2.0\"\nsource = {:?}\n",
            design_advisor.display().to_string()
        )
    );
    assert!(stdout(&output).contains("Updating .mcpctl.toml\n"), "stdout: {}", stdout(&output));
}

#[test]
fn setup_all_rerun_with_nothing_new_leaves_the_manifest_and_lock_untouched_and_reuses_installs() {
    let (workspace, project, _, _) = workspace_with_catalog();
    assert!(mcpctl(&workspace, &project, &["setup", "--all"]).status.success());
    let manifest = fs::read(project.join(".mcpctl.toml")).unwrap();
    let lock = fs::read(project.join(".mcpctl.lock")).unwrap();
    let installed_at = fs::metadata(workspace.join("state/packages/project-mcp/0.4.3")).unwrap().modified().unwrap();

    let output = mcpctl(&workspace, &project, &["setup", "--all"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(fs::read(project.join(".mcpctl.toml")).unwrap(), manifest);
    assert_eq!(fs::read(project.join(".mcpctl.lock")).unwrap(), lock);
    assert_eq!(fs::metadata(workspace.join("state/packages/project-mcp/0.4.3")).unwrap().modified().unwrap(), installed_at);
    assert!(!stdout(&output).contains(".mcpctl.toml\n"), "stdout: {}", stdout(&output));
    assert!(!stdout(&output).contains(".mcpctl.lock\n"), "stdout: {}", stdout(&output));
}

#[test]
fn setup_rerun_removes_deselected_catalog_mcps_from_the_manifest_and_lock_but_leaves_their_files() {
    let (workspace, project, project_mcp, _) = workspace_with_catalog();
    assert!(mcpctl(&workspace, &project, &["setup", "--all"]).status.success());
    let installed_at = fs::metadata(workspace.join("state/packages/project-mcp/0.4.3")).unwrap().modified().unwrap();

    let output = mcpctl(&workspace, &project, &["setup", "--mcp", "project-mcp"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(
        stdout(&output).contains("Removed design-advisor-mcp from .mcpctl.toml (scaffold files left in place)\n"),
        "stdout: {}",
        stdout(&output)
    );
    assert_eq!(
        fs::read_to_string(project.join(".mcpctl.toml")).unwrap(),
        format!(
            "[project]\nname = \"checkout-service\"\n\n[[mcp]]\nname = \"project-mcp\"\nversion = \"^0.4.3\"\nsource = {:?}\n",
            project_mcp.display().to_string()
        )
    );
    let lock = mcp_cli::project_lock::parse_project_lock(&fs::read_to_string(project.join(".mcpctl.lock")).unwrap()).unwrap();
    let locked: Vec<_> = lock.mcp.iter().map(|mcp| mcp.name.as_str()).collect();
    assert_eq!(locked, ["project-mcp"]);
    assert!(project.join("docs/design-advisor-mcp.md").exists());
    assert!(workspace.join("state/packages/design-advisor-mcp/0.2.0").exists());
    assert_eq!(fs::metadata(workspace.join("state/packages/project-mcp/0.4.3")).unwrap().modified().unwrap(), installed_at);
}

#[test]
fn setup_rerun_keeps_project_mcps_the_catalog_does_not_offer() {
    let (workspace, project, _, _) = workspace_with_catalog();
    write_package(&workspace, "local-mcp", "local-mcp", "1.0.0");
    let written = "[project]\nname = \"checkout-service\"\n\n[[mcp]]\nname = \"local-mcp\"\nversion = \"^1.0\"\nsource = \"../local-mcp\"\n";
    fs::write(project.join(".mcpctl.toml"), written).unwrap();

    let output = mcpctl(&workspace, &project, &["setup", "--mcp", "project-mcp"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(!stdout(&output).contains("Removed"), "stdout: {}", stdout(&output));
    let manifest = fs::read_to_string(project.join(".mcpctl.toml")).unwrap();
    assert!(manifest.starts_with(written), "{manifest}");
    let lock = mcp_cli::project_lock::parse_project_lock(&fs::read_to_string(project.join(".mcpctl.lock")).unwrap()).unwrap();
    let locked: Vec<_> = lock.mcp.iter().map(|mcp| mcp.name.as_str()).collect();
    assert_eq!(locked, ["local-mcp", "project-mcp"]);
}

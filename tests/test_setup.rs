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

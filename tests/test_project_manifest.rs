use std::{fs, path::PathBuf, sync::atomic::{AtomicUsize, Ordering}};

use mcp_cli::project::{parse_project_manifest, McpDeclaration};

fn temporary_dir() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-project-manifest-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn declaration(source: &str) -> McpDeclaration {
    McpDeclaration { name: "project-mcp".to_owned(), version: "^0.4".to_owned(), source: source.to_owned() }
}

#[test]
fn project_manifest_with_multiple_mcps_parses() {
    let manifest = parse_project_manifest(
        r#"
[project]
name = "checkout-service"

[[mcp]]
name = "project-mcp"
version = "^0.4"
source = "../project-mcp"

[[mcp]]
name = "outside-in-tdd-mcp"
version = "^1.3"
source = "../outside-in-tdd-mcp"
"#,
    )
    .unwrap();

    assert_eq!(manifest.project.name, "checkout-service");
    let names: Vec<&str> = manifest.mcp.iter().map(|mcp| mcp.name.as_str()).collect();
    assert_eq!(names, ["project-mcp", "outside-in-tdd-mcp"]);
    assert_eq!(manifest.mcp[0].version, "^0.4");
    assert_eq!(manifest.mcp[1].source, "../outside-in-tdd-mcp");
}

#[test]
fn project_manifest_missing_mcp_source_fails_clearly() {
    let error = parse_project_manifest(
        r#"
[project]
name = "checkout-service"

[[mcp]]
name = "project-mcp"
version = "^0.4"
"#,
    )
    .unwrap_err();

    assert!(error.contains("missing field `source`"), "{error}");
}

#[test]
fn project_manifest_rejects_invalid_version_constraint_naming_the_mcp() {
    let error = parse_project_manifest(
        r#"
[project]
name = "checkout-service"

[[mcp]]
name = "project-mcp"
version = "^0.4"
source = "../project-mcp"

[[mcp]]
name = "design-advisor-mcp"
version = "latest-please"
source = "../design-advisor-mcp"
"#,
    )
    .unwrap_err();

    assert!(error.contains("design-advisor-mcp"), "{error}");
    assert!(error.contains("latest-please"), "{error}");
}

#[test]
fn project_manifest_rejects_empty_version_constraint() {
    let error = parse_project_manifest(
        r#"
[project]
name = "checkout-service"

[[mcp]]
name = "project-mcp"
version = ""
source = "../project-mcp"
"#,
    )
    .unwrap_err();

    assert!(error.contains("project-mcp"), "{error}");
}

#[test]
fn project_manifest_rejects_duplicate_mcp_names() {
    let error = parse_project_manifest(
        r#"
[project]
name = "checkout-service"

[[mcp]]
name = "project-mcp"
version = "^0.4"
source = "../project-mcp"

[[mcp]]
name = "project-mcp"
version = "^0.5"
source = "../project-mcp-fork"
"#,
    )
    .unwrap_err();

    assert!(error.contains("duplicate"), "{error}");
    assert!(error.contains("project-mcp"), "{error}");
}

#[test]
fn project_manifest_rejects_non_adjacent_duplicate_mcp_names() {
    let error = parse_project_manifest(
        r#"
[project]
name = "checkout-service"

[[mcp]]
name = "project-mcp"
version = "^0.4"
source = "../project-mcp"

[[mcp]]
name = "outside-in-tdd-mcp"
version = "^1.3"
source = "../outside-in-tdd-mcp"

[[mcp]]
name = "project-mcp"
version = "^0.4"
source = "../project-mcp"
"#,
    )
    .unwrap_err();

    assert!(error.contains("duplicate mcp 'project-mcp'"), "{error}");
}

#[test]
fn missing_local_source_fails_naming_the_mcp_and_resolved_path() {
    let root = temporary_dir().join("checkout-service");
    fs::create_dir_all(&root).unwrap();

    let error = declaration("../project-mcp").resolve_local_source(&root).unwrap_err();

    assert!(error.contains("project-mcp"), "{error}");
    assert!(error.contains(&root.join("../project-mcp").display().to_string()), "{error}");
}

#[test]
fn local_source_without_package_manifest_fails_clearly() {
    let root = temporary_dir();
    fs::create_dir_all(root.join("project-mcp")).unwrap();

    let error = declaration("project-mcp").resolve_local_source(&root).unwrap_err();

    assert!(error.contains("project-mcp"), "{error}");
    assert!(error.contains("mcpctl.toml"), "{error}");
}

#[test]
fn local_source_that_is_a_file_fails_clearly() {
    let root = temporary_dir();
    fs::write(root.join("project-mcp"), "not a directory").unwrap();

    let error = declaration("project-mcp").resolve_local_source(&root).unwrap_err();

    assert!(error.contains("not a directory"), "{error}");
}

#[test]
fn valid_local_source_resolves_relative_to_project_root() {
    let workspace = temporary_dir();
    let root = workspace.join("checkout-service");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(workspace.join("project-mcp")).unwrap();
    fs::write(workspace.join("project-mcp/mcpctl.toml"), "").unwrap();

    let resolved = declaration("../project-mcp").resolve_local_source(&root).unwrap();

    assert_eq!(resolved, root.join("../project-mcp"));
}

#[test]
fn project_manifest_without_any_mcp_entries_parses_as_declaring_none() {
    let manifest = parse_project_manifest("[project]\nname = \"empty-service\"\n").unwrap();

    assert_eq!(manifest.project.name, "empty-service");
    assert!(manifest.mcp.is_empty());
}

#[test]
fn project_manifest_renders_the_project_table_then_one_mcp_table_per_declaration_and_parses_back() {
    let manifest = mcp_cli::project::ProjectManifest {
        project: mcp_cli::project::ProjectInfo { name: "checkout-service".to_owned() },
        mcp: vec![
            McpDeclaration { name: "design-advisor-mcp".to_owned(), version: "^0.2.0".to_owned(), source: "/code/design-advisor-mcp".to_owned() },
            declaration("../project-mcp"),
        ],
    };

    let rendered = mcp_cli::project::render_project_manifest(&manifest);

    assert_eq!(
        rendered,
        "[project]\nname = \"checkout-service\"\n\n\
         [[mcp]]\nname = \"design-advisor-mcp\"\nversion = \"^0.2.0\"\nsource = \"/code/design-advisor-mcp\"\n\n\
         [[mcp]]\nname = \"project-mcp\"\nversion = \"^0.4\"\nsource = \"../project-mcp\"\n"
    );
    assert_eq!(parse_project_manifest(&rendered).unwrap(), manifest);
}

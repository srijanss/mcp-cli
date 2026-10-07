use mcp_cli::project::parse_project_manifest;

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

use mcp_cli::manifest::{parse_binary_manifest, parse_manifest, Runtime};

#[test]
fn valid_python_manifest_parses_into_package_model() {
    let manifest = parse_manifest(
        r#"
name = "project-mcp"
version = "0.4.0"
description = "Project knowledge MCP"

[runtime]
type = "python"
python = ">=3.12,<3.13"

[install]
strategy = "uv"
entrypoint = "project-mcp"
"#,
    )
    .unwrap();

    assert_eq!(manifest.name, "project-mcp");
    assert_eq!(manifest.version, "0.4.0");
    assert_eq!(manifest.install.entrypoint, "project-mcp");
    assert!(matches!(manifest.runtime, Runtime::Python { python } if python == ">=3.12,<3.13"));
}

#[test]
fn python_manifest_preserves_description_and_install_strategy() {
    let manifest = parse_manifest(
        r#"
name = "project-mcp"
version = "0.4.0"
description = "Project knowledge MCP"

[runtime]
type = "python"
python = ">=3.12,<3.13"

[install]
strategy = "uv"
entrypoint = "project-mcp"
"#,
    )
    .unwrap();

    assert_eq!(manifest.description.as_deref(), Some("Project knowledge MCP"));
    assert_eq!(manifest.install.strategy.as_deref(), Some("uv"));
}

#[test]
fn valid_binary_manifest_parses_into_binary_package_model() {
    let manifest = parse_binary_manifest(
        r#"
name = "example-rust-mcp"
version = "1.0.0"

[runtime]
type = "binary"

[install]
entrypoint = "example-rust-mcp"
"#,
    )
    .unwrap();

    assert_eq!(manifest.name, "example-rust-mcp");
    assert!(matches!(manifest.runtime, Runtime::Binary));
}

#[test]
fn binary_parser_rejects_python_manifest() {
    assert!(parse_binary_manifest(
        "name = \"x\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"python\"\npython = \">=3.12\"\n[install]\nentrypoint = \"x\""
    )
    .is_err());
}

#[test]
fn manifest_rejects_invalid_semver() {
    assert!(parse_manifest(
        "name = \"project-mcp\"\nversion = \"not-a-version\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"project-mcp\""
    )
    .is_err());
}

#[test]
fn manifest_rejects_empty_name() {
    assert!(parse_manifest(
        "name = \"\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"project-mcp\""
    )
    .is_err());
}

#[test]
fn manifest_rejects_path_like_name() {
    assert!(parse_manifest(
        "name = \"../../evil\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"evil\""
    )
    .is_err());
}

#[test]
fn manifest_rejects_name_with_separator() {
    assert!(parse_manifest(
        "name = \"project/mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"mcp\""
    )
    .is_err());
}

#[test]
fn python_manifest_requires_uv_install_strategy() {
    assert!(parse_manifest(
        "name = \"project-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"python\"\npython = \">=3.12\"\n[install]\nentrypoint = \"project-mcp\""
    )
    .is_err());
}

#[test]
fn python_manifest_rejects_non_uv_install_strategy() {
    assert!(parse_manifest(
        "name = \"project-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"python\"\npython = \">=3.12\"\n[install]\nstrategy = \"pip\"\nentrypoint = \"project-mcp\""
    )
    .is_err());
}

#[test]
fn python_manifest_rejects_empty_runtime_constraint() {
    assert!(parse_manifest(
        "name = \"project-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"python\"\npython = \"\"\n[install]\nstrategy = \"uv\"\nentrypoint = \"project-mcp\""
    )
    .is_err());
}

#[test]
fn python_manifest_rejects_whitespace_runtime_constraint() {
    assert!(parse_manifest(
        "name = \"project-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"python\"\npython = \"  \"\n[install]\nstrategy = \"uv\"\nentrypoint = \"project-mcp\""
    )
    .is_err());
}

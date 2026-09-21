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

fn binary_manifest(name: &str, version: &str) -> String {
    format!(
        "name = \"{name}\"\nversion = \"{version}\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"x\""
    )
}

#[test]
fn manifest_rejects_uppercase_name() {
    assert!(parse_manifest(&binary_manifest("Project-MCP", "1.0.0")).is_err());
}

#[test]
fn manifest_rejects_name_starting_with_separator_char() {
    for name in ["-mcp", ".mcp", "_mcp"] {
        assert!(parse_manifest(&binary_manifest(name, "1.0.0")).is_err(), "{name}");
    }
}

#[test]
fn manifest_accepts_lowercase_names_with_dots_underscores_and_hyphens() {
    for name in ["mcp", "0mcp", "my.mcp_server-2"] {
        assert!(parse_manifest(&binary_manifest(name, "1.0.0")).is_ok(), "{name}");
    }
}

#[test]
fn manifest_rejects_semver_build_metadata() {
    assert!(parse_manifest(&binary_manifest("project-mcp", "1.0.0+local")).is_err());
}

#[test]
fn manifest_accepts_semver_prerelease() {
    assert!(parse_manifest(&binary_manifest("project-mcp", "1.0.0-rc.1")).is_ok());
}

fn binary_manifest_with_entrypoint(entrypoint: &str) -> String {
    format!(
        "name = \"example-rust-mcp\"\nversion = \"1.0.0\"\n[runtime]\ntype = \"binary\"\n[install]\nentrypoint = \"{entrypoint}\""
    )
}

#[test]
fn binary_manifest_accepts_source_relative_entrypoint_paths() {
    for entrypoint in ["example-rust-mcp", "dist/example-rust-mcp", "target/release/bin/x", "./dist/x"] {
        let manifest = parse_manifest(&binary_manifest_with_entrypoint(entrypoint))
            .unwrap_or_else(|error| panic!("{entrypoint}: {error}"));
        assert_eq!(manifest.install.entrypoint, entrypoint);
    }
}

#[test]
fn binary_manifest_rejects_entrypoints_that_name_a_directory_with_dot_segment() {
    for entrypoint in [".", "./", "dist/.", "./."] {
        assert!(
            parse_manifest(&binary_manifest_with_entrypoint(entrypoint)).is_err(),
            "{entrypoint:?}"
        );
    }
}

#[test]
fn binary_manifest_rejects_traversal_absolute_and_empty_entrypoints() {
    for entrypoint in ["", "/bin/sh", "../outside", "dist/../../outside", "dist/..", "..", "dist//x", "dist/", "C:\\\\x", "dist\\\\x"] {
        assert!(
            parse_manifest(&binary_manifest_with_entrypoint(entrypoint)).is_err(),
            "{entrypoint:?}"
        );
    }
}

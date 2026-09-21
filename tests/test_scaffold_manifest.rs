use mcp_cli::manifest::parse_manifest;

const BASE: &str = r#"name = "scaf-mcp"
version = "1.0.0"

[runtime]
type = "binary"

[install]
entrypoint = "scaf-mcp"
"#;

fn with_scaffold(scaffold: &str) -> String {
    format!("{BASE}\n{scaffold}")
}

#[test]
fn scaffold_section_parses_files_dirs_excludes_and_hints() {
    let manifest = parse_manifest(&with_scaffold(
        r#"[scaffold]
dirs = [".agents", ".claude"]
exclude = [".claude/settings.local.json"]

[[scaffold.files]]
from = ".mcp.json.example"
to = ".mcp.json"

[[scaffold.hints]]
message = "Defaults to pytest."

[[scaffold.hints]]
when_exists = "Cargo.toml"
message = "Rust project: use cargo-adapter-runner."
"#,
    ))
    .unwrap();

    let scaffold = manifest.scaffold.expect("scaffold section should be parsed");
    assert_eq!(scaffold.dirs, [".agents", ".claude"]);
    assert_eq!(scaffold.exclude, [".claude/settings.local.json"]);
    assert_eq!(scaffold.files.len(), 1);
    assert_eq!(scaffold.files[0].from, ".mcp.json.example");
    assert_eq!(scaffold.files[0].to, ".mcp.json");
    assert_eq!(scaffold.hints.len(), 2);
    assert_eq!(scaffold.hints[0].when_exists, None);
    assert_eq!(scaffold.hints[0].message, "Defaults to pytest.");
    assert_eq!(scaffold.hints[1].when_exists.as_deref(), Some("Cargo.toml"));
}

#[test]
fn scaffold_paths_that_escape_the_package_or_target_are_rejected() {
    let unsafe_scaffolds = [
        "[scaffold]\ndirs = [\"../outside\"]\n",
        "[scaffold]\ndirs = [\"/etc\"]\n",
        "[scaffold]\nexclude = [\"a/../../b\"]\n",
        "[[scaffold.files]]\nfrom = \"../secret\"\nto = \".mcp.json\"\n",
        "[[scaffold.files]]\nfrom = \"template.json\"\nto = \"../../.bashrc\"\n",
        "[[scaffold.files]]\nfrom = \"template.json\"\nto = \"/tmp/x\"\n",
        "[[scaffold.hints]]\nwhen_exists = \"../Cargo.toml\"\nmessage = \"x\"\n",
    ];

    for scaffold in unsafe_scaffolds {
        let error = parse_manifest(&with_scaffold(scaffold)).expect_err(scaffold);
        assert!(error.contains("scaffold"), "error should mention scaffold, got: {error} (for {scaffold:?})");
    }
}

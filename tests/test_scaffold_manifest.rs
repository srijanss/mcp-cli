use mcp_cli::manifest::{parse_manifest, MergeMode};

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

#[test]
fn scaffold_files_accept_a_json_or_toml_merge_mode_and_reject_anything_else() {
    let manifest = parse_manifest(&with_scaffold(
        r#"[[scaffold.files]]
from = "a.json"
to = ".claude/settings.json"
merge = "json"

[[scaffold.files]]
from = "b.toml"
to = ".codex/config.toml"
merge = "toml"

[[scaffold.files]]
from = "c.txt"
to = "c.txt"
"#,
    ))
    .unwrap();

    let files = manifest.scaffold.unwrap().files;
    assert_eq!(files[0].merge, Some(MergeMode::Json));
    assert_eq!(files[1].merge, Some(MergeMode::Toml));
    assert_eq!(files[2].merge, None);

    let error = parse_manifest(&with_scaffold("[[scaffold.files]]\nfrom = \"a\"\nto = \"b\"\nmerge = \"yaml\"\n")).expect_err("yaml is not a merge mode");
    assert!(error.contains("yaml") || error.contains("merge"), "error was: {error}");
}

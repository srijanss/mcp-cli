use std::fs;

#[test]
fn readme_gives_a_new_developer_a_complete_mcpctl_handoff() {
    let readme = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))
        .expect("README.md should document mcpctl");

    for section in [
        "# mcpctl",
        "## Why mcpctl",
        "## Install",
        "## MCP runtime vs project runtime",
        "## Configuration (`mcpctl.toml`)",
        "## Python MCPs and uv",
        "## Native binary MCPs",
        "## Command reference",
        "## MCP client configuration",
        "## Docker",
        "## macOS",
        "## Troubleshooting",
        "## `mcpctl doctor`",
    ] {
        assert!(readme.contains(section), "README should include {section}");
    }

    for example in [
        "MCPCTL_HOME",
        "mcpctl install",
        "mcpctl run",
        "mcpctl --version",
        ".mcp.json",
        "Claude",
        "OpenCode",
        "Pi",
        "/workspace/.venv",
    ] {
        assert!(readme.contains(example), "README should explain or show {example}");
    }
}

#[test]
fn readme_documents_portable_checksum_verification() {
    let readme = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))
        .expect("README.md should document checksum verification");

    assert!(readme.contains("shasum -a 256 -c SHA256SUMS"));
    assert!(readme.contains("sha256sum -c SHA256SUMS"));
}

#[test]
fn readme_locks_down_executable_install_and_run_examples() {
    let readme = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))
        .expect("README.md should document executable examples");

    for command in [
        "MCPCTL_HOME=/state mcpctl install ./legacy-mcp",
        "MCPCTL_HOME=/state mcpctl install ./modern-mcp",
        "mcpctl run <name>[@<version>] [server-arguments...]",
        "mcpctl use <name>@<version>",
    ] {
        assert!(readme.contains(command), "README should include {command}");
    }
}

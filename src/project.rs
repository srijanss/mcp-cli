use serde::Deserialize;

/// A project's `.mcpctl.toml`: which MCPs the project uses and at what versions.
#[derive(Debug, Deserialize, PartialEq)]
pub struct ProjectManifest {
    pub project: ProjectInfo,
    pub mcp: Vec<McpDeclaration>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct ProjectInfo {
    pub name: String,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct McpDeclaration {
    pub name: String,
    pub version: String,
    pub source: String,
}

pub fn parse_project_manifest(contents: &str) -> Result<ProjectManifest, String> {
    let manifest: ProjectManifest = toml::from_str(contents).map_err(|error| error.to_string())?;
    for mcp in &manifest.mcp {
        semver::VersionReq::parse(&mcp.version).map_err(|error| {
            format!("mcp '{}' has invalid version constraint '{}': {error}", mcp.name, mcp.version)
        })?;
    }
    Ok(manifest)
}

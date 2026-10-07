use std::path::{Path, PathBuf};

use serde::Deserialize;

/// A project's `.mcpctl.toml`: which MCPs the project uses and at what versions.
#[derive(Debug, Deserialize, PartialEq)]
pub struct ProjectManifest {
    pub project: ProjectInfo,
    #[serde(default)]
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

impl McpDeclaration {
    /// Resolves `source` against the project root and checks it is a local package directory.
    pub fn resolve_local_source(&self, project_root: &Path) -> Result<PathBuf, String> {
        let path = project_root.join(&self.source);
        if !path.is_dir() {
            return Err(format!(
                "mcp '{}' source {} does not exist or is not a directory",
                self.name,
                path.display()
            ));
        }
        if !path.join("mcpctl.toml").is_file() {
            return Err(format!("mcp '{}' source {} has no mcpctl.toml", self.name, path.display()));
        }
        Ok(path)
    }
}

pub fn parse_project_manifest(contents: &str) -> Result<ProjectManifest, String> {
    let manifest: ProjectManifest = toml::from_str(contents).map_err(|error| error.to_string())?;
    let mut seen = std::collections::HashSet::new();
    for mcp in &manifest.mcp {
        if !seen.insert(mcp.name.as_str()) {
            return Err(format!("duplicate mcp '{}' declared more than once", mcp.name));
        }
        semver::VersionReq::parse(&mcp.version).map_err(|error| {
            format!("mcp '{}' has invalid version constraint '{}': {error}", mcp.name, mcp.version)
        })?;
    }
    Ok(manifest)
}

/// Walks up from `start` to the nearest directory holding `.mcpctl.toml`, stopping at the filesystem root.
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join(".mcpctl.toml").is_file())
        .map(Path::to_path_buf)
}

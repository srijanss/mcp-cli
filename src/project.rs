use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A project's `.mcpctl.toml`: which MCPs the project uses and at what versions.
#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct ProjectManifest {
    pub project: ProjectInfo,
    #[serde(default)]
    pub mcp: Vec<McpDeclaration>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct ProjectInfo {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
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

pub fn render_project_manifest(manifest: &ProjectManifest) -> String {
    toml::to_string(manifest).expect("project manifest fields are plain strings")
}

/// `contents` of a `.mcpctl.toml` with the `removed` MCPs' tables dropped and `added` declared as new `[[mcp]]`
/// tables, leaving everything else as written. An inline `mcp = [...]` array is turned into `[[mcp]]` tables
/// first, since tables cannot be appended to it.
pub fn edit_project_manifest(contents: &str, added: &[McpDeclaration], removed: &[String]) -> Result<String, String> {
    let mut document: toml_edit::DocumentMut = contents.parse().map_err(|error: toml_edit::TomlError| error.to_string())?;
    let (key, existing) = document.remove_entry("mcp").map_or((None, None), |(key, item)| (Some(key), Some(item)));
    let mut tables = match existing {
        None => toml_edit::ArrayOfTables::new(),
        // toml_edit will not convert an empty inline array, so it is replaced outright.
        Some(toml_edit::Item::Value(toml_edit::Value::Array(array))) if array.is_empty() => toml_edit::ArrayOfTables::new(),
        Some(existing) => existing.into_array_of_tables().map_err(|_| "mcp must be a list of tables".to_owned())?,
    };
    tables.retain(|table| !table.get("name").and_then(toml_edit::Item::as_str).is_some_and(|name| removed.iter().any(|removed| removed == name)));
    for declaration in added {
        let mut table = toml_edit::Table::new();
        table["name"] = toml_edit::value(declaration.name.as_str());
        table["version"] = toml_edit::value(declaration.version.as_str());
        table["source"] = toml_edit::value(declaration.source.as_str());
        tables.push(table);
    }
    // Comments above an inline `mcp = ...` key would be lost with it, so they move to the first `[[mcp]]` table.
    let comments = key.and_then(|key| key.leaf_decor().prefix().and_then(|prefix| prefix.as_str()).map(str::to_owned));
    if let (Some(comments), Some(first)) = (comments.filter(|comments| !comments.trim().is_empty()), tables.get_mut(0)) {
        first.decor_mut().set_prefix(format!("\n{}", comments.trim_start_matches('\n')));
    }
    document.insert("mcp", tables.into());
    Ok(document.to_string())
}

/// Walks up from `start` to the nearest directory holding `.mcpctl.toml`, stopping at the filesystem root.
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join(".mcpctl.toml").is_file())
        .map(Path::to_path_buf)
}

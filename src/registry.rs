use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Registry {
    pub schema_version: u32,
    pub packages: std::collections::BTreeMap<String, PackageRecord>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct PackageRecord {
    pub active_version: Option<String>,
    pub versions: Vec<InstalledVersion>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct InstalledVersion {
    pub version: String,
    pub runtime: String,
    pub source: String,
    pub installed_at: String,
}

impl Registry {
    pub fn new() -> Self {
        Self { schema_version: 1, packages: std::collections::BTreeMap::new() }
    }

    pub fn with_package(name: &str, package: PackageRecord) -> Self {
        let mut registry = Self::new();
        registry.packages.insert(name.to_owned(), package);
        registry
    }
}

pub fn save_registry(path: &Path, registry: &Registry) -> Result<(), String> {
    let contents = serde_json::to_string_pretty(registry).map_err(|error| error.to_string())?;
    std::fs::write(path, contents).map_err(|error| error.to_string())
}

pub fn load_registry(path: &Path) -> Result<Registry, String> {
    let contents = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let registry: Registry = serde_json::from_str(&contents)
        .map_err(|error| format!("invalid registry {}: {error}", path.display()))?;
    if registry.schema_version != 1 {
        return Err(format!(
            "unsupported registry schema {} in {}",
            registry.schema_version,
            path.display()
        ));
    }
    Ok(registry)
}

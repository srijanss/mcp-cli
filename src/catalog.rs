use serde::{Deserialize, Serialize};

/// The user's `catalog.toml`: local MCP package sources `setup` can offer, in the order they were added.
#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Catalog {
    #[serde(default)]
    pub mcp: Vec<CatalogEntry>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct CatalogEntry {
    pub source: String,
}

pub fn parse_catalog(contents: &str) -> Result<Catalog, String> {
    toml::from_str(contents).map_err(|error| error.to_string())
}

pub fn render_catalog(catalog: &Catalog) -> String {
    toml::to_string(catalog).expect("catalog entries are plain strings")
}

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "mcpctl", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Install an MCP from a local project path
    Install {
        #[arg(value_name = "project path")]
        project: String,
    },
    /// Run an installed MCP
    Run {
        package: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        arguments: Vec<String>,
    },
    /// List installed MCPs
    List,
    /// Show details for an installed MCP
    Info { package: String },
    /// Uninstall an MCP
    Uninstall { package: String },
    /// Select the active version of an MCP
    Use { selector: String },
    /// Update an MCP from a local source
    Update {
        package: String,
        #[arg(long)]
        source: String,
    },
    /// Check the local installation for problems
    Doctor,
    /// Copy an installed MCP's scaffold files into a project, or every project MCP's when none is named
    Init {
        package: Option<String>,
        #[arg(value_name = "target directory")]
        target: Option<String>,
    },
    /// Lock and install the MCPs declared in the project's .mcpctl.toml
    Sync {
        /// Install exactly the versions in .mcpctl.lock without rewriting it
        #[arg(long)]
        locked: bool,
    },
    /// Manage the local catalog of MCP sources that setup offers
    Catalog {
        #[command(subcommand)]
        command: CatalogCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum CatalogCommand {
    /// Add a local MCP package source to the catalog
    Add {
        #[arg(value_name = "package path")]
        path: String,
    },
    /// List the MCPs in the catalog
    List,
    /// Remove an MCP from the catalog by name or source path
    Remove {
        #[arg(value_name = "name or path")]
        target: String,
    },
}

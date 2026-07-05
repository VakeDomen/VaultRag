use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "vaultfind", about = "RAG over your Obsidian vault")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Ensure Qdrant is running and collection exists
    Init,

    /// Index an Obsidian vault into Qdrant
    Index {
        /// Path to the Obsidian vault (overrides config)
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Query the indexed vault
    Query {
        /// The query string
        #[arg(short, long)]
        query: String,

        /// Number of results to return
        #[arg(short, long, default_value = "5")]
        n: usize,

        /// Path to the Obsidian vault (overrides config)
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Tear down the Qdrant container and volume
    Teardown,

    /// Get or set config values
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// List all valid config keys
    List,
    /// Get a config value (omit key to print all)
    Get {
        /// Dot-separated key, e.g. vault.path, qdrant.grpc_port
        key: Option<String>,
    },
    /// Set a config value
    Set {
        /// Dot-separated key, e.g. vault.path
        key: String,
        /// Value to set
        value: String,
    },
}

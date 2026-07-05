use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "vaultrag", about = "RAG over your Obsidian vault")]
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
        /// Path to the Obsidian vault
        #[arg(short, long)]
        path: String,
    },

    /// Query the indexed vault
    Query {
        /// The query string
        #[arg(short, long)]
        query: String,

        /// Number of results to return
        #[arg(short, long, default_value = "5")]
        n: usize,
    },

    /// Tear down the Qdrant container and volume
    Teardown,

    /// Print current config path
    ConfigPath,
}

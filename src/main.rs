mod cli;
mod config;
mod qdrant;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;

    match cli.command {
        Commands::Init => {
            println!("Config: {}", Config::config_path()?.display());
            let _client = qdrant::ensure_qdrant(&config).await?;
            println!("Ready.");
        }
        Commands::Index { path } => {
            let _client = qdrant::ensure_qdrant(&config).await?;
            println!("Indexing vault at {}...", path);
            // TODO: implement indexing
        }
        Commands::Query { query, n } => {
            let _client = qdrant::ensure_qdrant(&config).await?;
            println!("Querying: {} (top {})", query, n);
            // TODO: implement query
        }
        Commands::Teardown => {
            qdrant::teardown(&config)?;
            println!("Done.");
        }
        Commands::ConfigPath => {
            println!("{}", Config::config_path()?.display());
        }
    }

    Ok(())
}

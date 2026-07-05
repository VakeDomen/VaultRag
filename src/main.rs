mod chunker;
mod cli;
mod config;
mod embed;
mod hash_tree;
mod hype;
mod index;
mod qdrant;

use anyhow::{bail, Context, Result};
use clap::Parser;
use cli::{Cli, Commands, ConfigAction};
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut config = Config::load()?;

    match cli.command {
        Commands::Init => {
            println!("Config: {}", Config::config_path()?.display());
            let _client = qdrant::ensure_qdrant(&config).await?;
            println!("Ready.");
        }
        Commands::Index { path } => {
            let vault_path = resolve_vault_path(&config, path)?;
            let client = qdrant::ensure_qdrant(&config).await?;
            index::index(&config, &vault_path, &client).await?;
        }
        Commands::Query { query, n, path } => {
            let _vault_path = resolve_vault_path(&config, path)?;
            let _client = qdrant::ensure_qdrant(&config).await?;
            println!("Querying: {} (top {})", query, n);
            // TODO: implement query
        }
        Commands::Teardown => {
            qdrant::teardown(&config)?;
            println!("Done.");
        }
        Commands::Config { action } => match action {
            ConfigAction::List => {
                for key in [
                    "vault.path",
                    "llm.provider",
                    "llm.model",
                    "llm.base_url",
                    "llm.api_key",
                    "chunking.max_chunk_words",
                    "chunking.parallelism",
                    "qdrant.host",
                    "qdrant.grpc_port",
                    "qdrant.rest_port",
                    "qdrant.collection_name",
                    "qdrant.docker_container_name",
                    "qdrant.docker_volume_name",
                    "qdrant.docker_image",
                    "embedding.provider",
                    "embedding.model",
                    "embedding.base_url",
                    "embedding.api_key",
                    "embedding.dimension",
                ] {
                    println!("{} = {}", key, config.get(key)?);
                }
            }
            ConfigAction::Get { key } => match key {
                Some(k) => println!("{}", config.get(&k)?),
                None => println!(
                    "{}",
                    toml::to_string_pretty(&config)
                        .context("failed to serialize config")?
                ),
            },
            ConfigAction::Set { key, value } => {
                let value = if key == "vault.path" {
                    std::path::Path::new(&value)
                        .canonicalize()
                        .context("invalid vault path")?
                        .to_string_lossy()
                        .to_string()
                } else {
                    value
                };
                config.set(&key, &value)?;
                config.save()?;
                println!("Set {} = {}", key, value);
            }
        },
    }

    Ok(())
}

fn resolve_vault_path(config: &Config, cli_path: Option<String>) -> Result<String> {
    if let Some(p) = cli_path {
        return Ok(p);
    }
    if let Some(p) = &config.vault.path {
        return Ok(p.clone());
    }
    bail!("no vault path set. Use --path or 'vaultfind config set vault.path <path>'")
}

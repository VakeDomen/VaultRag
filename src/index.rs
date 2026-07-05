use crate::config::Config;
use anyhow::{bail, Context, Result};
use qdrant_client::qdrant::CountPointsBuilder;
use qdrant_client::Qdrant;
use std::io::Write;
use std::path::Path;
use walkdir::WalkDir;

/// Validate vault, discover files, check collection state, then index.
pub async fn index(config: &Config, vault_path: &str, client: &Qdrant) -> Result<()> {
    validate_vault_path(vault_path)?;

    let files = discover_md_files(vault_path)?;

    check_collection_state(config, client).await?;

    println!("Found {} markdown files to index.", files.len());
    // TODO: chunking, embedding, upsert

    Ok(())
}

fn validate_vault_path(vault_path: &str) -> Result<()> {
    let path = Path::new(vault_path);

    if !path.exists() {
        bail!("vault path does not exist: {vault_path}");
    }
    if !path.is_dir() {
        bail!("vault path is not a directory: {vault_path}");
    }

    // Check readability
    path.read_dir()
        .with_context(|| format!("permission denied reading vault: {vault_path}"))?;

    // Warn if .obsidian is missing
    if !path.join(".obsidian").exists() {
        eprintln!("warning: no .obsidian directory found — are you sure this is an Obsidian vault?");
    }

    Ok(())
}

fn discover_md_files(vault_path: &str) -> Result<Vec<String>> {
    let mut files = Vec::new();

    for entry in WalkDir::new(vault_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            // Skip hidden directories (except the vault root itself)
            if e.depth() == 0 {
                return true;
            }
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                return !name.starts_with('.') && name != "node_modules";
            }
            true
        })
    {
        let entry = entry.with_context(|| format!("failed to walk vault at {vault_path}"))?;

        if entry.file_type().is_file() {
            let name = entry.file_name().to_string_lossy();
            if name.ends_with(".md") {
                files.push(entry.path().to_string_lossy().to_string());
            }
        }
    }

    files.sort();

    if files.is_empty() {
        bail!("no markdown files found in vault: {vault_path}");
    }

    if files.len() > 10_000 {
        eprintln!(
            "warning: found {} markdown files — this may take a while",
            files.len()
        );
    }

    Ok(files)
}

async fn check_collection_state(config: &Config, client: &Qdrant) -> Result<()> {
    let collection_name = &config.qdrant.collection_name;

    let count = client
        .count(CountPointsBuilder::new(collection_name))
        .await
        .context("failed to count points in collection")?;

    let count = count.result.map(|r| r.count as u64).unwrap_or(0);

    if count > 0 {
        eprint!(
            "Collection '{}' already has {} point(s). Re-index? [y/N] ",
            collection_name, count
        );
        let _ = std::io::stdout().flush();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        if input != "y" && input != "yes" {
            println!("Aborting.");
            std::process::exit(0);
        }
    }

    Ok(())
}

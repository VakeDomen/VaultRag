use crate::chunker;
use crate::config::Config;
use crate::embed::Embedder;
use crate::hash_tree::HashTree;
use crate::hype::HyPEGenerator;
use crate::qdrant;
use anyhow::{bail, Context, Result};
use qdrant_client::Qdrant;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Semaphore;
use walkdir::WalkDir;

pub async fn index(config: &Config, vault_path: &str, client: &Qdrant) -> Result<()> {
    validate_vault_path(vault_path)?;
    let files = discover_md_files(vault_path)?;

    let config_dir = Config::config_dir()?;
    let prev_tree = HashTree::load(&config_dir)?;
    let (to_index, to_remove, skipped) = HashTree::diff(&files, vault_path, &prev_tree)?;

    println!(
        "{} to index, {} to remove, {} unchanged",
        to_index.len(),
        to_remove.len(),
        skipped
    );

    if to_remove.is_empty() && to_index.is_empty() {
        println!("Nothing to do.");
        return Ok(());
    }

    // Remove points for deleted files
    for path in &to_remove {
        let name = Path::new(path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        if let Err(e) = qdrant::delete_by_note_path(client, config, path).await {
            eprintln!("  {name}: failed to remove — {e:#}");
        }
    }

    // Index new/changed files
    let semaphore = Arc::new(Semaphore::new(config.chunking.parallelism));
    let config = Arc::new(config.clone());
    let client = client.clone();
    let mut tree = prev_tree;

    let mut handles = Vec::new();

    for file_path in to_index {
        let permit = semaphore.clone().acquire_owned().await;
        let config = config.clone();
        let client = client.clone();

        handles.push(tokio::spawn(async move {
            let _permit = permit;
            // Remove old points first (no-op if file is new)
            let _ = qdrant::delete_by_note_path(&client, &config, &file_path).await;
            let result = process_file(&config, &client, &file_path).await;
            (file_path, result)
        }));
    }

    for handle in handles {
        let (file_path, result) = handle.await?;
        if let Err(e) = result {
            let name = Path::new(&file_path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            eprintln!("  {name}: error — {e:#}");
            continue;
        }

        // Update hash tree entry
        let vault = Path::new(vault_path);
        if let Some(rel) = pathdiff::diff_paths(&file_path, vault) {
            let rel = rel.to_string_lossy().to_string();
            if let Ok(hash) = HashTree::compute_hash(&file_path) {
                if let Ok(mtime_ns) = HashTree::mtime_nanos(&file_path) {
                    tree.files.insert(
                        rel,
                        crate::hash_tree::FileEntry { hash, mtime_ns },
                    );
                }
            }
        }
    }

    // Remove deleted files from tree
    for path in &to_remove {
        let vault = Path::new(vault_path);
        if let Some(rel) = pathdiff::diff_paths(path, vault) {
            tree.files.remove(&rel.to_string_lossy().to_string());
        }
    }

    tree.save(&config_dir)?;

    println!();
    println!("Done.");

    Ok(())
}

async fn process_file(
    config: &Config,
    client: &Qdrant,
    file_path: &str,
) -> Result<(usize, usize)> {
    let meta = match std::fs::metadata(file_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("  {}: skipping — {e}", Path::new(file_path).file_name().unwrap_or_default().to_string_lossy());
            return Ok((0, 0));
        }
    };
    if meta.len() > config.chunking.max_file_bytes as u64 {
        let name = Path::new(file_path).file_name().map(|s| s.to_string_lossy()).unwrap_or_default();
        eprintln!("  {name}: skipping — file too large ({} bytes, limit {})", meta.len(), config.chunking.max_file_bytes);
        return Ok((0, 0));
    }

    let chunks = chunker::chunk_file(file_path, config.chunking.max_chunk_words)
        .context("failed to chunk file")?;

    if chunks.is_empty() {
        return Ok((0, 0));
    }

    let mut all_points = Vec::new();

    for chunk in &chunks {
        let questions = HyPEGenerator::generate(config, &chunk.text).await?;
        let refs: Vec<&str> = questions.iter().map(|s| s.as_str()).collect();
        let embeddings = Embedder::embed_batch(config, &refs).await?;

        for (question, embedding) in questions.into_iter().zip(embeddings) {
            all_points.push((embedding, question, chunk.clone()));
        }
    }

    let total_questions = all_points.len();
    qdrant::upsert_chunks(client, config, all_points).await?;

    let name = Path::new(file_path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    println!("  {name}: {} chunks, {total_questions} questions", chunks.len());

    Ok((chunks.len(), total_questions))
}

fn validate_vault_path(vault_path: &str) -> Result<()> {
    let path = Path::new(vault_path);

    if !path.exists() {
        bail!("vault path does not exist: {vault_path}");
    }
    if !path.is_dir() {
        bail!("vault path is not a directory: {vault_path}");
    }

    path.read_dir()
        .with_context(|| format!("permission denied reading vault: {vault_path}"))?;

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
                // Skip excalidraw drawings (they're large JSON blobs, not text)
                if name.ends_with(".excalidraw.md") {
                    continue;
                }
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

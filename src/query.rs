use crate::config::Config;
use crate::embed::Embedder;
use crate::qdrant;
use anyhow::Result;
use qdrant_client::Qdrant;
use std::path::Path;

fn get_str<'a>(p: &'a qdrant_client::qdrant::ScoredPoint, key: &str) -> &'a str {
    p.get(key)
        .as_str()
        .map(|s| s.as_str())
        .unwrap_or("")
}

pub async fn query(
    config: &Config,
    vault_path: &str,
    client: &Qdrant,
    query_text: &str,
    n: usize,
) -> Result<()> {
    let vector = Embedder::embed(config, query_text).await?;
    let results = qdrant::search(client, config, vector, n as u64).await?;

    let vault = Path::new(vault_path);

    for p in &results {
        let note_path = get_str(p, "note_path");
        let rel_path = Path::new(note_path)
            .strip_prefix(vault)
            .unwrap_or(Path::new(note_path))
            .to_string_lossy();

        let title = get_str(p, "note_title");
        let section = get_str(p, "section");
        let section = if section.is_empty() { "__root__" } else { section };

        println!("{:.2}  {}", p.score, title);
        println!("      {}  \u{bb}  {}", rel_path, section);
        println!();
    }

    Ok(())
}

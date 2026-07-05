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

fn extract_snippet(text: &str, max_chars: usize) -> String {
    let text = text.trim();
    if text.len() <= max_chars {
        return text.to_string();
    }
    // Try to cut at a sentence boundary
    let truncated = &text[..max_chars];
    if let Some(last_period) = truncated.rfind('.') {
        if last_period > max_chars / 2 {
            return format!("{}...", &truncated[..=last_period].trim());
        }
    }
    format!("{}...", truncated.trim())
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
        let text = get_str(p, "text");
        let snippet = extract_snippet(text, 200);

        println!("{:.2}  {}", p.score, title);
        println!("      {}", rel_path);
        println!("      \u{2192} {}", snippet);
        println!();
    }

    Ok(())
}

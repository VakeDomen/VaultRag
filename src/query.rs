use crate::config::Config;
use crate::embed::Embedder;
use crate::qdrant;
use anyhow::{Context, Result};
use qdrant_client::Qdrant;
use std::collections::BTreeMap;
use std::fs;

fn get_str<'a>(p: &'a qdrant_client::qdrant::ScoredPoint, key: &str) -> &'a str {
    p.get(key)
        .as_str()
        .map(|s| s.as_str())
        .unwrap_or("")
}

fn get_f64(p: &qdrant_client::qdrant::ScoredPoint, key: &str) -> Option<f64> {
    p.get(key).as_double()
}

fn extract_snippet(text: &str, max_chars: usize) -> String {
    let text = text.trim();
    if text.len() <= max_chars {
        return text.to_string();
    }
    let truncated = &text[..max_chars];
    if let Some(last_period) = truncated.rfind('.') {
        if last_period > max_chars / 2 {
            return format!("{}...", &truncated[..=last_period].trim());
        }
    }
    format!("{}...", truncated.trim())
}

fn read_lines(path: &str, start: usize, end: usize) -> Result<String> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path))?;
    let lines: Vec<&str> = content.lines().collect();
    let end = end.min(lines.len().saturating_sub(1));
    if start > end {
        return Ok(String::new());
    }
    Ok(lines[start..=end].join("\n"))
}

// ANSI color helpers
mod c {
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const CYAN: &str = "\x1b[36m";
    pub const GREEN: &str = "\x1b[32m";
    pub const RESET: &str = "\x1b[0m";
}

pub async fn query(
    config: &Config,
    _vault_path: &str,
    client: &Qdrant,
    query_text: &str,
    n: usize,
) -> Result<()> {
    let fetch = (n * 10) as u64;
    let vector = Embedder::embed(config, query_text).await?;
    let results = qdrant::search(client, config, vector, fetch).await?;

    // Group by note_path, deduplicate by chunk_id (keep highest score)
    let mut files: BTreeMap<String, Vec<&qdrant_client::qdrant::ScoredPoint>> = BTreeMap::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for p in &results {
        let chunk_id = get_str(p, "chunk_id").to_string();
        if !seen.insert(chunk_id) {
            continue;
        }
        let note_path = get_str(p, "note_path").to_string();
        files.entry(note_path).or_default().push(p);
    }

    // Score each file by its top chunk, take top n files
    let mut file_scores: Vec<(f32, String)> = files
        .iter()
        .map(|(path, chunks)| (chunks.first().map(|c| c.score).unwrap_or(0.0), path.clone()))
        .collect();
    file_scores.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    file_scores.truncate(n);

    for (i, (file_score, file_path)) in file_scores.iter().enumerate() {
        if i > 0 {
            println!("{}══════════════════════════════════════════════════{}", c::DIM, c::RESET);
        }

        let title = files[file_path]
            .first()
            .map(|p| get_str(p, "note_title"))
            .unwrap_or("");

        let score_pct = (file_score * 100.0) as u8;

        println!("{}{}{}  {}──{}  {}{}% match{}",
            c::BOLD, title, c::RESET, c::DIM, c::RESET, c::GREEN, score_pct, c::RESET);
        println!("  {}{}{}", c::DIM, file_path, c::RESET);
        println!();

        for (j, p) in files[file_path].iter().enumerate() {
            if j > 0 {
                println!("  {}──{}", c::DIM, c::RESET);
            }

            let section = get_str(p, "section");
            let section = if section.is_empty() { "__root__" } else { section };

            let note_path = get_str(p, "note_path");
            let start = get_f64(p, "start_line").unwrap_or(0.0) as usize;
            let end = get_f64(p, "end_line").unwrap_or(0.0) as usize;
            let chunk_text = read_lines(note_path, start, end).unwrap_or_default();
            let snippet = extract_snippet(&chunk_text, 300);

            println!("  {}[{}]{}  {}L{}-L{}{}",
                c::CYAN, section, c::RESET,
                c::DIM, start + 1, end + 1, c::RESET);
            println!("  {}", snippet);
        }
    }

    Ok(())
}

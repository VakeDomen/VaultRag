use anyhow::Result;
use std::fs;

#[derive(Debug, Clone)]
pub struct Chunk {
    pub chunk_id: String,
    pub note_path: String,
    pub note_title: String,
    pub section: Option<String>,
    pub chunk_index: usize,
    pub total_chunks_in_section: usize,
    pub text: String,
    pub tags: Vec<String>,
}

pub fn chunk_file(path: &str, max_chunk_words: usize) -> Result<Vec<Chunk>> {
    let content = fs::read_to_string(path)?;
    let (frontmatter_tags, body) = parse_frontmatter(&content);
    let note_path = path.to_string();
    let note_title = extract_title(&body, path);

    let chunks = split_hierarchical(body, max_chunk_words);

    let total = chunks.len();
    let mut result = Vec::new();
    for (i, (section, text)) in chunks.iter().enumerate() {
        let chunk_id = format!(
            "{}::{}::{}",
            note_path,
            section.as_deref().unwrap_or("__root__"),
            i
        );
        result.push(Chunk {
            chunk_id,
            note_path: note_path.clone(),
            note_title: note_title.clone(),
            section: section.clone(),
            chunk_index: i,
            total_chunks_in_section: total,
            text: text.clone(),
            tags: frontmatter_tags.clone(),
        });
    }

    Ok(result)
}

pub fn chunk_all_files(files: &[String], max_chunk_words: usize) -> Result<Vec<Chunk>> {
    let mut all = Vec::new();
    for path in files {
        let chunks = chunk_file(path, max_chunk_words)?;
        all.extend(chunks);
    }
    Ok(all)
}

fn parse_frontmatter(content: &str) -> (Vec<String>, &str) {
    let content = content.trim_start();
    if let Some(rest) = content.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            let front = &rest[..end];
            let tags: Vec<String> = front
                .lines()
                .filter_map(|l| {
                    let l = l.trim();
                    if let Some(val) = l.strip_prefix("tags:") {
                        let val = val.trim();
                        if val.starts_with('[') && val.ends_with(']') {
                            Some(
                                val[1..val.len() - 1]
                                    .split(',')
                                    .map(|t| t.trim().trim_matches('"').trim_matches('\'').to_string())
                                    .collect::<Vec<_>>(),
                            )
                        } else {
                            None
                        }
                    } else if l.starts_with('-') {
                        let t = l.trim_start_matches('-').trim();
                        if !t.is_empty() { Some(vec![t.to_string()]) } else { None }
                    } else {
                        None
                    }
                })
                .flatten()
                .collect();
            let body = rest[end + 5..].trim_start();
            return (tags, body);
        }
    }
    (Vec::new(), content)
}

fn extract_title(body: &str, path: &str) -> String {
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with("# ") {
            return t[2..].trim().to_string();
        }
    }
    std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "untitled".to_string())
}

fn split_hierarchical(body: &str, max_words: usize) -> Vec<(Option<String>, String)> {
    let mut result = Vec::new();
    let mut current_h2: Option<String> = None;
    let mut current_h3: Option<String> = None;
    let mut buffer = Vec::new();

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## ") && !trimmed.starts_with("### ") {
            flush_buffer(&mut buffer, &current_h2, &current_h3, max_words, &mut result);
            current_h2 = Some(trimmed[3..].trim().to_string());
            current_h3 = None;
        } else if trimmed.starts_with("### ") {
            flush_buffer(&mut buffer, &current_h2, &current_h3, max_words, &mut result);
            current_h3 = Some(trimmed[4..].trim().to_string());
        } else {
            buffer.push(line);
        }
    }
    flush_buffer(&mut buffer, &current_h2, &current_h3, max_words, &mut result);

    if result.is_empty() {
        result.push((None, body.to_string()));
    }

    result
}

fn flush_buffer(
    buffer: &mut Vec<&str>,
    h2: &Option<String>,
    h3: &Option<String>,
    max_words: usize,
    result: &mut Vec<(Option<String>, String)>,
) {
    if buffer.is_empty() {
        return;
    }
    let text = buffer.join("\n").trim().to_string();
    buffer.clear();
    if text.is_empty() {
        return;
    }

    let section_name = h3.clone().or_else(|| h2.clone());

    if word_count(&text) <= max_words {
        result.push((section_name, text));
        return;
    }

    // Still too big — split by paragraphs
    let paragraphs: Vec<&str> = text
        .split("\n\n")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

    if paragraphs.len() == 1 {
        // Single paragraph too big — split by sentences
        for chunk in split_by_sentences(&text, max_words) {
            result.push((section_name.clone(), chunk));
        }
    } else {
        for para in paragraphs {
            if word_count(para) <= max_words {
                result.push((section_name.clone(), para.to_string()));
            } else {
                for chunk in split_by_sentences(para, max_words) {
                    result.push((section_name.clone(), chunk));
                }
            }
        }
    }
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

fn split_by_sentences(text: &str, max_words: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_words = 0;

    for sentence in split_sentences(text) {
        let sw = word_count(&sentence);
        if current_words + sw > max_words && !current.is_empty() {
            chunks.push(current.trim().to_string());
            current = String::new();
            current_words = 0;
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(&sentence);
        current_words += sw;
    }

    if !current.is_empty() {
        chunks.push(current.trim().to_string());
    }

    if chunks.is_empty() {
        chunks.push(text.to_string());
    }

    chunks
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        current.push(ch);
        if ch == '.' || ch == '!' || ch == '?' {
            sentences.push(current.trim().to_string());
            current.clear();
        }
    }

    if !current.trim().is_empty() {
        sentences.push(current.trim().to_string());
    }

    sentences
}

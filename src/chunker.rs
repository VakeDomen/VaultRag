use anyhow::Result;
use std::fs;

#[derive(Debug, Clone)]
pub struct Chunk {
    pub chunk_id: String,
    pub note_path: String,
    pub note_title: String,
    pub section: Option<String>,
    pub section_hierarchy: Vec<String>,
    pub chunk_index: usize,
    pub total_chunks_in_section: usize,
    pub text: String,
    pub tags: Vec<String>,
    pub file_type: String,
}

#[derive(Debug)]
enum Block {
    CodeFence(String),
    Table(String),
    Blockquote(String),
    List(String),
    Heading { level: usize, text: String },
    Paragraph(String),
}

const TAU_MIN: usize = 100;
const TAU_MAX: usize = 1500;

pub fn chunk_file(path: &str, _max_chunk_words: usize) -> Result<Vec<Chunk>> {
    let content = fs::read_to_string(path)?;
    let (frontmatter_tags, body) = parse_frontmatter(&content);
    let note_path = path.to_string();
    let note_title = extract_title(&body, path);
    let file_type = std::path::Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();

    let blocks = parse_blocks(body);
    let raw_chunks = build_chunks(&blocks);

    let mut result = Vec::new();
    for (i, (hierarchy, text)) in raw_chunks.iter().enumerate() {
        let section = hierarchy.last().cloned();
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
            section,
            section_hierarchy: hierarchy.clone(),
            chunk_index: i,
            total_chunks_in_section: raw_chunks.len(),
            text: text.clone(),
            tags: frontmatter_tags.clone(),
            file_type: file_type.clone(),
        });
    }

    Ok(result)
}

/// Parse markdown into typed blocks.
fn parse_blocks(body: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = body.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Code fence
        if let Some(fence) = fence_match(line) {
            let mut code = String::from(line);
            i += 1;
            while i < lines.len() && !fence_match(lines[i]).is_some_and(|f| f == fence) {
                code.push('\n');
                code.push_str(lines[i]);
                i += 1;
            }
            if i < lines.len() {
                code.push('\n');
                code.push_str(lines[i]);
                i += 1;
            }
            blocks.push(Block::CodeFence(code));
            continue;
        }

        // Heading
        if let Some(level) = heading_level(line) {
            let text = line[level..].trim().to_string();
            blocks.push(Block::Heading { level, text });
            i += 1;
            continue;
        }

        // Blockquote
        if line.trim_start().starts_with('>') {
            let mut quote = String::new();
            while i < lines.len() && lines[i].trim_start().starts_with('>') {
                if !quote.is_empty() {
                    quote.push('\n');
                }
                quote.push_str(lines[i]);
                i += 1;
            }
            blocks.push(Block::Blockquote(quote));
            continue;
        }

        // Table (line contains | and is a separator or data row)
        if is_table_line(line) {
            let mut table = String::new();
            while i < lines.len() && is_table_line(lines[i]) {
                if !table.is_empty() {
                    table.push('\n');
                }
                table.push_str(lines[i]);
                i += 1;
            }
            blocks.push(Block::Table(table));
            continue;
        }

        // List
        if is_list_line(line) {
            let mut list = String::new();
            while i < lines.len() && is_list_line(lines[i]) {
                if !list.is_empty() {
                    list.push('\n');
                }
                list.push_str(lines[i]);
                i += 1;
            }
            blocks.push(Block::List(list));
            continue;
        }

        // Paragraph (collect consecutive non-empty, non-special lines)
        let mut para = String::new();
        while i < lines.len() {
            let l = lines[i];
            if l.trim().is_empty() {
                break;
            }
            if fence_match(l).is_some() || heading_level(l).is_some()
                || l.trim_start().starts_with('>')
                || is_table_line(l) || is_list_line(l)
            {
                break;
            }
            if !para.is_empty() {
                para.push('\n');
            }
            para.push_str(l);
            i += 1;
        }
        if !para.is_empty() {
            blocks.push(Block::Paragraph(para));
        } else {
            // Empty line — skip
            i += 1;
        }
    }

    blocks
}

fn fence_match(line: &str) -> Option<&'static str> {
    let trimmed = line.trim();
    if trimmed.starts_with("```") {
        Some("```")
    } else if trimmed.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    }
}

fn heading_level(line: &str) -> Option<usize> {
    let trimmed = line.trim();
    if !trimmed.starts_with('#') {
        return None;
    }
    let level = trimmed.chars().take_while(|c| *c == '#').count();
    if level > 6 {
        return None;
    }
    if trimmed.as_bytes().get(level).copied() != Some(b' ') {
        return None;
    }
    Some(level)
}

fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('|') && trimmed.chars().filter(|c| *c == '|').count() >= 2
}

fn is_list_line(line: &str) -> bool {
    let trimmed = line.trim();
    // Unordered: -, *, +
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
        return true;
    }
    // Ordered: 1. 2. etc
    if let Some(rest) = trimmed.strip_suffix('.') {
        if rest.chars().all(|c| c.is_ascii_digit()) && !rest.is_empty() {
            // Check it's actually at the start (after optional whitespace)
            let prefix_len = trimmed.len() - rest.len() - 1;
            let before = &trimmed[..prefix_len];
            return before.chars().all(|c| c.is_whitespace());
        }
    }
    false
}

/// Build chunks from blocks using τmin/τmax and header stack.
fn build_chunks(blocks: &[Block]) -> Vec<(Vec<String>, String)> {
    let mut chunks: Vec<(Vec<String>, String)> = Vec::new();
    let mut header_stack: Vec<(usize, String)> = Vec::new();
    let mut current_blocks: Vec<String> = Vec::new();
    let mut current_size: usize = 0;

    fn section_title(stack: &[(usize, String)]) -> Vec<String> {
        stack.iter().map(|(_, t)| t.clone()).collect()
    }

    fn flush(chunks: &mut Vec<(Vec<String>, String)>, stack: &[(usize, String)], blocks: &mut Vec<String>, _size: &mut usize) {
        if blocks.is_empty() {
            return;
        }
        let text = blocks.join("\n\n");
        blocks.clear();
        *_size = 0;
        let hierarchy = section_title(stack);
        // Skip if text is only whitespace
        if text.trim().is_empty() {
            return;
        }
        chunks.push((hierarchy, text));
    }

    for block in blocks {
        match block {
            Block::Heading { level, text } => {
                // Flush current chunk before new heading
                flush(&mut chunks, &header_stack, &mut current_blocks, &mut current_size);

                // Update header stack
                while let Some(&(top_level, _)) = header_stack.last() {
                    if top_level >= *level {
                        header_stack.pop();
                    } else {
                        break;
                    }
                }
                header_stack.push((*level, text.clone()));

                // Heading starts a new chunk (heading text is included)
                current_blocks.push(block_text(block));
                current_size = block_text(block).len();
            }
            _ => {
                let text = block_text(block);
                let block_size = text.len();

                if current_blocks.is_empty() {
                    // First block in chunk
                    current_blocks.push(text);
                    current_size = block_size;
                } else if current_size + 2 + block_size <= TAU_MAX {
                    // Fits
                    current_blocks.push(text);
                    current_size += 2 + block_size;
                } else if current_size >= TAU_MIN {
                    // Current chunk is big enough, flush it and start new
                    flush(&mut chunks, &header_stack, &mut current_blocks, &mut current_size);
                    current_blocks.push(text);
                    current_size = block_size;
                } else {
                    // Below τmin, keep growing (oversized chunk)
                    current_blocks.push(text);
                    current_size += 2 + block_size;
                }
            }
        }
    }

    flush(&mut chunks, &header_stack, &mut current_blocks, &mut current_size);

    chunks
}

fn block_text(block: &Block) -> String {
    match block {
        Block::CodeFence(t) => t.clone(),
        Block::Table(t) => t.clone(),
        Block::Blockquote(t) => t.clone(),
        Block::List(t) => t.clone(),
        Block::Heading { text, .. } => {
            // Reconstruct heading with # markers for context
            let level = match block {
                Block::Heading { level, .. } => *level,
                _ => unreachable!(),
            };
            format!("{} {}", "#".repeat(level), text)
        }
        Block::Paragraph(t) => t.clone(),
    }
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

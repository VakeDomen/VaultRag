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
    pub start_line: usize,
    pub end_line: usize,
    pub tags: Vec<String>,
    pub file_type: String,
}

struct LocatedBlock {
    block: Block,
    start_line: usize,
    end_line: usize,
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
    let (frontmatter_tags, body, fm_lines) = parse_frontmatter(&content);
    let note_path = path.to_string();
    let note_title = extract_title(&body, path);
    let file_type = std::path::Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();

    let blocks = parse_blocks(body);
    let raw_chunks = build_chunks(&blocks);

    let mut result = Vec::new();
    for (i, (hierarchy, text, start, end)) in raw_chunks.iter().enumerate() {
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
            start_line: start + fm_lines,
            end_line: end + fm_lines,
            tags: frontmatter_tags.clone(),
            file_type: file_type.clone(),
        });
    }

    Ok(result)
}

/// Parse markdown into typed blocks with line ranges.
fn parse_blocks(body: &str) -> Vec<LocatedBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = body.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let start = i;

        // Code fence
        if let Some(fence) = fence_match(line) {
            i += 1;
            while i < lines.len() && !fence_match(lines[i]).is_some_and(|f| f == fence) {
                i += 1;
            }
            if i < lines.len() {
                i += 1;
            }
            let end = i - 1;
            let text = lines[start..=end].join("\n");
            blocks.push(LocatedBlock { block: Block::CodeFence(text), start_line: start, end_line: end });
            continue;
        }

        // Heading
        if let Some(level) = heading_level(line) {
            let text = line[level..].trim().to_string();
            blocks.push(LocatedBlock { block: Block::Heading { level, text }, start_line: start, end_line: start });
            i += 1;
            continue;
        }

        // Blockquote
        if line.trim_start().starts_with('>') {
            i += 1;
            while i < lines.len() && lines[i].trim_start().starts_with('>') {
                i += 1;
            }
            let end = i - 1;
            let text = lines[start..=end].join("\n");
            blocks.push(LocatedBlock { block: Block::Blockquote(text), start_line: start, end_line: end });
            continue;
        }

        // Table
        if is_table_line(line) {
            i += 1;
            while i < lines.len() && is_table_line(lines[i]) {
                i += 1;
            }
            let end = i - 1;
            let text = lines[start..=end].join("\n");
            blocks.push(LocatedBlock { block: Block::Table(text), start_line: start, end_line: end });
            continue;
        }

        // List
        if is_list_line(line) {
            i += 1;
            while i < lines.len() && is_list_line(lines[i]) {
                i += 1;
            }
            let end = i - 1;
            let text = lines[start..=end].join("\n");
            blocks.push(LocatedBlock { block: Block::List(text), start_line: start, end_line: end });
            continue;
        }

        // Paragraph
        i += 1;
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
            i += 1;
        }
        let end = i - 1;
        if start <= end {
            let text = lines[start..=end].join("\n");
            blocks.push(LocatedBlock { block: Block::Paragraph(text), start_line: start, end_line: end });
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
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
        return true;
    }
    if let Some(rest) = trimmed.strip_suffix('.') {
        if rest.chars().all(|c| c.is_ascii_digit()) && !rest.is_empty() {
            let prefix_len = trimmed.len() - rest.len() - 1;
            let before = &trimmed[..prefix_len];
            return before.chars().all(|c| c.is_whitespace());
        }
    }
    false
}

fn build_chunks(blocks: &[LocatedBlock]) -> Vec<(Vec<String>, String, usize, usize)> {
    let mut chunks = Vec::new();
    let mut header_stack: Vec<(usize, String)> = Vec::new();
    let mut current_blocks: Vec<(String, usize, usize)> = Vec::new();
    let mut current_size: usize = 0;

    fn flush(
        chunks: &mut Vec<(Vec<String>, String, usize, usize)>,
        stack: &[(usize, String)],
        current: &mut Vec<(String, usize, usize)>,
        size: &mut usize,
    ) {
        if current.is_empty() {
            return;
        }
        let start = current.first().map(|(_, s, _)| *s).unwrap_or(0);
        let end = current.last().map(|(_, _, e)| *e).unwrap_or(0);
        let text = current.iter().map(|(t, _, _)| t.as_str()).collect::<Vec<_>>().join("\n\n");
        current.clear();
        *size = 0;
        if text.trim().is_empty() {
            return;
        }
        let hierarchy: Vec<String> = stack.iter().map(|(_, t)| t.clone()).collect();
        chunks.push((hierarchy, text, start, end));
    }

    for lb in blocks {
        match &lb.block {
            Block::Heading { level, text } => {
                flush(&mut chunks, &header_stack, &mut current_blocks, &mut current_size);

                while let Some(&(top_level, _)) = header_stack.last() {
                    if top_level >= *level {
                        header_stack.pop();
                    } else {
                        break;
                    }
                }
                header_stack.push((*level, text.clone()));

                let bt = block_text(&lb.block);
                current_blocks.push((bt, lb.start_line, lb.end_line));
                current_size = current_blocks.last().map(|(t, _, _)| t.len()).unwrap_or(0);
            }
            _ => {
                let bt = block_text(&lb.block);
                let block_size = bt.len();

                if current_blocks.is_empty() {
                    current_blocks.push((bt, lb.start_line, lb.end_line));
                    current_size = block_size;
                } else if current_size + 2 + block_size <= TAU_MAX {
                    current_blocks.push((bt, lb.start_line, lb.end_line));
                    current_size += 2 + block_size;
                } else if current_size >= TAU_MIN {
                    flush(&mut chunks, &header_stack, &mut current_blocks, &mut current_size);
                    current_blocks.push((bt, lb.start_line, lb.end_line));
                    current_size = block_size;
                } else {
                    current_blocks.push((bt, lb.start_line, lb.end_line));
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
            let level = match block {
                Block::Heading { level, .. } => *level,
                _ => unreachable!(),
            };
            format!("{} {}", "#".repeat(level), text)
        }
        Block::Paragraph(t) => t.clone(),
    }
}

fn parse_frontmatter(content: &str) -> (Vec<String>, &str, usize) {
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
            // Count frontmatter lines: opening --- + front + closing ---
            let fm_lines = 1 + front.lines().count() + 1;
            return (tags, body, fm_lines);
        }
    }
    (Vec::new(), content, 0)
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

use std::collections::HashMap;

/// Tokenize text into lowercase words with basic stemming (English).
fn tokenize(text: &str) -> Vec<String> {
    text.split_ascii_whitespace()
        .filter(|w| w.len() >= 2)
        .map(|w| {
            let cleaned: String = w
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '\'' || *c == '-')
                .collect::<String>()
                .to_lowercase();
            // Simple English stemming: remove common suffixes
            let stemmed = clean_suffix(&cleaned);
            stemmed
        })
        .filter(|w| w.len() >= 2 && !is_stopword(w))
        .collect()
}

fn clean_suffix(w: &str) -> String {
    let w = w.trim_end_matches('\'');
    let w = w.trim_end_matches("'s");
    // Porter-lite suffixes
    let w = w.trim_end_matches("ingly");
    let w = w.trim_end_matches("tion");
    let w = w.trim_end_matches("ment");
    let w = w.trim_end_matches("ness");
    let w = w.trim_end_matches("able");
    let w = w.trim_end_matches("ible");
    let w = w.trim_end_matches("ship");
    let w = w.trim_end_matches("hood");
    let w = w.trim_end_matches("less");
    let w = w.trim_end_matches("wise");
    let w = w.trim_end_matches("ical");
    let w = w.trim_end_matches("ity");
    let w = w.trim_end_matches("ive");
    let w = w.trim_end_matches("ful");
    let w = w.trim_end_matches("ous");
    let w = w.trim_end_matches("ist");
    let w = w.trim_end_matches("ism");
    let w = w.trim_end_matches("ize");
    let w = w.trim_end_matches("ise");
    let w = w.trim_end_matches("ify");
    let w = w.trim_end_matches("ate");
    let w = w.trim_end_matches("ing");
    let w = w.trim_end_matches("ied");
    let w = w.trim_end_matches("ies");
    let w = w.trim_end_matches("ed");
    let w = w.trim_end_matches("es");
    let w = w.trim_end_matches("er");
    let w = w.trim_end_matches("est");
    let w = w.trim_end_matches("ly");
    let w = w.trim_end_matches("s");
    w.to_string()
}

fn is_stopword(w: &str) -> bool {
    matches!(
        w,
        "a" | "an"
            | "as"
            | "at"
            | "be"
            | "by"
            | "do"
            | "go"
            | "he"
            | "if"
            | "in"
            | "is"
            | "it"
            | "me"
            | "my"
            | "no"
            | "of"
            | "on"
            | "or"
            | "so"
            | "to"
            | "up"
            | "us"
            | "we"
            | "am"
            | "the"
            | "and"
            | "for"
            | "are"
            | "but"
            | "not"
            | "you"
            | "all"
            | "can"
            | "has"
            | "was"
            | "had"
            | "her"
            | "his"
            | "its"
            | "out"
            | "did"
            | "get"
            | "may"
            | "say"
            | "she"
            | "too"
            | "use"
            | "way"
            | "who"
            | "now"
            | "any"
            | "how"
            | "own"
            | "new"
            | "old"
            | "see"
            | "two"
            | "boy"
            | "set"
            | "put"
            | "end"
            | "let"
            | "try"
            | "ask"
            | "man"
            | "lot"
            | "run"
            | "pay"
            | "cut"
            | "ago"
            | "far"
            | "god"
            | "yet"
            | "got"
            | "few"
            | "big"
            | "top"
            | "bad"
            | "low"
            | "job"
            | "add"
            | "win"
            | "buy"
            | "die"
            | "fee"
            | "bit"
            | "bar"
            | "cup"
            | "age"
            | "per"
            | "etc"
            | "via"
            | "pro"
            | "non"
            | "pre"
            | "sub"
            | "iii"
            | "ii"
            | "al"
            | "el"
            | "la"
            | "le"
            | "en"
            | "un"
            | "de"
            | "du"
            | "des"
            | "das"
            | "der"
            | "ein"
            | "eine"
            | "und"
            | "oder"
            | "aber"
            | "sich"
    )
}

/// Hash a token string to a u32 index for use in sparse vectors.
fn hash_token(token: &str) -> u32 {
    let mut h = 2166136261u32;
    for b in token.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

/// Compute a sparse BM25 vector (TF only) from text.
/// Qdrant's IDF modifier will apply IDF weighting at query time.
pub fn compute_sparse_vector(text: &str) -> Vec<(u32, f32)> {
    let tokens = tokenize(text);
    let mut freq: HashMap<u32, f32> = HashMap::new();
    for token in &tokens {
        let h = hash_token(token);
        *freq.entry(h).or_insert(0.0) += 1.0;
    }
    let mut result: Vec<(u32, f32)> = freq.into_iter().collect();
    result.sort_by_key(|(idx, _)| *idx);
    result
}

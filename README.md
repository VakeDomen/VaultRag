# VaultFind

Semantic + BM25 hybrid search over an Obsidian vault using **HyPE** (Hypothetical Prompt Embeddings). Each chunk of a markdown file is passed to an LLM which generates hypothetical questions a user might ask about that content. Those questions are embedded (dense vectors) and stored in Qdrant alongside client-computed BM25 sparse vectors. At query time, both searches run in parallel and are fused via Reciprocal Rank Fusion (RRF) — question-to-question semantic matching plus exact keyword retrieval.

Only the LLM-generated questions, BM25 sparse vectors, and metadata (path, line range, section) are stored in Qdrant. The original text is read from disk at query time to produce snippets.

## Quickstart

```bash
# Initialize Qdrant (Docker) and create the collection
vaultfind init

# Index your vault
vaultfind index --path /path/to/your/vault

# Or set the path once and omit --path afterwards
vaultfind config set vault.path /path/to/your/vault
vaultfind index

# Search
vaultfind query "what are world models?"
```

## Commands

### `vaultfind init`

Starts a Qdrant Docker container (if not running) and creates the collection. Safe to re-run.

### `vaultfind index`

Scans the vault for `.md` files, computes file hashes, and incrementally indexes only new or changed files. Skips unchanged files using mtime + SHA-256.

```bash
# Index with default parallelism (4)
vaultfind index

# Override vault path for this run
vaultfind index --path /path/to/vault
```

The hash tree is stored at `~/.config/vaultfind/hash_tree.json`. Deleted files are automatically removed from Qdrant.

### `vaultfind query`

```bash
vaultfind query "natural language query"
vaultfind query -n 10 "gamma correction"
```

Arguments:
- `query` (positional) — the search query
- `-n` — number of file results (default: 5)

Internally fetches `n × 10` candidates from Qdrant via hybrid search (dense + BM25 sparse fused with RRF), deduplicates by `chunk_id`, groups by file, and returns the top `n` files with all their matching chunks.

### `vaultfind config`

```bash
# List all config values
vaultfind config list

# Get a specific value
vaultfind config get llm.model

# Set a value
vaultfind config set embedding.model bge-m3

# Toggle BM25 hybrid search
vaultfind config set bm25.enabled false
```

### `vaultfind teardown`

Stops the Qdrant container, removes the Docker volume, and deletes the hash tree. Destructive.

## Configuration

Auto-created at `~/.config/vaultfind/config.toml`:

```toml
[vault]
path = "/path/to/your/vault"

[llm]
provider = "openai"
model = "diffusiongemma"
base_url = "https://your-endpoint/v1"
api_key = "..."

[chunking]
max_chunk_words = 512
parallelism = 10

[qdrant]
host = "localhost"
grpc_port = 6339
rest_port = 6338
collection_name = "vault_chunks"
docker_container_name = "vaultfind-qdrant"
docker_volume_name = "vaultfind_data"
docker_image = "qdrant/qdrant:latest"

[bm25]
enabled = true

[embedding]
provider = "openai"
model = "embeddinggemma"
base_url = "https://your-endpoint/v1"
api_key = "..."
dimension = 768
```

Both LLM and embedding configs support `provider`, `base_url`, and `api_key`. Supported providers: `ollama`, `openai`, `openrouter`.

## How it works

### Indexing

1. Walk vault, find `.md` files
2. Compare mtime + SHA-256 against previous run (hash tree)
3. New/changed files:
   - Delete old Qdrant points for that file
   - Parse markdown into typed blocks (code fences, tables, blockquotes, lists, headings, paragraphs) — **never** split code blocks, tables, lists, or blockquotes
   - Build chunks using τmin (100 chars) / τmax (1500 chars) thresholds with a header stack for section hierarchy
   - For each chunk, call LLM to generate hypothetical questions
   - Embed each question, upsert into Qdrant with question text, file path, and line range
4. Remove Qdrant points for deleted files
5. Save updated hash tree

### Querying

1. Embed the query string using the configured embedding model
2. If BM25 is enabled (default): run two prefetch queries in parallel — dense vector search (cosine similarity) and BM25 sparse vector search — fused via `Fusion::Rrf`
3. If BM25 is disabled: fall back to dense vector search only
4. Deduplicate by `chunk_id`
5. Group by file, take top `n` files
5. For each chunk, read the original text from disk using stored line range
6. Print results with score, file path, section, question, and snippet

## Output format

```
World Models  ──  85% match
  /home/.../world-models.md

  [__root__]  L1-L15
  Q: What are world models and how do they work?
  # World Models\n\nWorld models are internal models of the environment...

  ──
  [Architecture]  L16-L42
  Q: How do world models enable planning in latent space?
  ### Architecture\n- Encoder maps observations to latent states...

══════════════════════════════════════════════════

LeWorldModel (LeWM)  ──  78% match
  /home/.../leworldmodel.md

  [Connections]  L1-L8
  Q: How does JEPA relate to world models?
  - [[jepa]] — Joint Embedding Predictive Architecture framework...
```

## Hybrid Search (BM25)

BM25 lexical search is enabled by default and runs alongside the dense semantic search. A client-side BM25 tokenizer (English stemming + stopword filtering) computes term-frequency sparse vectors from each chunk's text. These are stored as Qdrant sparse vectors (`"chunk_bm25"`) with server-side IDF weighting. At query time, both searches run as parallel prefetches and results are fused via **Reciprocal Rank Fusion (RRF)**.

Disable with: `vaultfind config set bm25.enabled false`

## Storage

Each Qdrant point stores:
- `question` — the generated hypothetical question
- `chunk_id` — unique chunk identifier
- `note_path` — absolute file path
- `note_title` — file title
- `file_type` — file extension
- `section` — leaf section heading
- `section_hierarchy` — full heading path
- `start_line`, `end_line` — line range in the file (0-indexed)
- `chunk_index`, `total_chunks_in_section`
- `tags` — frontmatter tags

No chunk text is stored in Qdrant. Text is read from disk at query time using the line range.

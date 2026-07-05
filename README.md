# VaultFind

Semantic search over an Obsidian vault using HyPE (Hypothetical Prompt Embeddings). Each chunk of a markdown file is passed to an LLM which generates hypothetical questions a user might ask about that content. Those questions are embedded and stored in Qdrant. At query time, your query is embedded directly and matched against the question vectors — question-to-question search.

No text is indexed directly; only the LLM-generated questions. The original chunk text is stored alongside for snippets.

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

Internally fetches `n × 10` candidates from Qdrant, deduplicates by chunk, groups by file, and returns the top `n` files with all their matching chunks.

### `vaultfind config`

```bash
# List all config values
vaultfind config list

# Get a specific value
vaultfind config get llm.model

# Set a value
vaultfind config set embedding.model bge-m3
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
   - Split file hierarchically: `##` → `###` → paragraphs → sentences (recursive, only if chunk exceeds `max_chunk_words`)
   - For each chunk, call LLM to generate hypothetical questions
   - Embed all questions, upsert into Qdrant
4. Remove Qdrant points for deleted files
5. Save updated hash tree

### Querying

1. Embed the query string using the configured embedding model
2. Search Qdrant (cosine similarity) — fetches `n × 10` candidates
3. Deduplicate by `chunk_id`
4. Group by file, take top `n` files
5. Print results with score, file path, section, and snippet

## Output format

```
**World Models**  ──  85% match
  /home/.../world-models.md

  [__root__]
  # World Models\n\nWorld models are internal models of the environment...

══════════════════════════════════════════════════

**LeWorldModel (LeWM)**  ──  78% match
  /home/.../leworldmodel.md

  [Connections]
  - [[jepa]] — Joint Embedding Predictive Architecture...

  ──
  [Architecture]
  ### Components\n- **Encoder:** ViT-Tiny...
```

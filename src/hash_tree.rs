use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub hash: String,
    pub mtime_ns: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashTree {
    pub files: HashMap<String, FileEntry>,
}

impl HashTree {
    pub fn load(config_dir: &Path) -> Result<Self> {
        let path = config_dir.join("hash_tree.json");
        if path.exists() {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(Self {
                files: HashMap::new(),
            })
        }
    }

    pub fn save(&self, config_dir: &Path) -> Result<()> {
        let path = config_dir.join("hash_tree.json");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }

    pub fn compute_hash(path: &str) -> Result<String> {
        let content = fs::read(path)?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let hash = hasher.finalize();
        Ok(hash.iter().map(|b| format!("{:02x}", b)).collect())
    }

    pub fn mtime_nanos(path: &str) -> Result<i64> {
        let meta = fs::metadata(path)?;
        let mtime = meta.modified()?;
        let duration = mtime
            .duration_since(std::time::UNIX_EPOCH)
            .context("file mtime before unix epoch")?;
        Ok(duration.as_nanos() as i64)
    }

    /// Diff current files against the previous hash tree.
    ///
    /// Returns `(to_index: Vec<absolute_path>, to_remove: Vec<absolute_path>, skipped: usize)`.
    ///
    /// `vault_path` is used to compute relative paths for hash tree lookup.
    /// `files` are absolute paths.
    pub fn diff(
        files: &[String],
        vault_path: &str,
        previous: &HashTree,
    ) -> Result<(Vec<String>, Vec<String>, usize)> {
        let vault = Path::new(vault_path);
        let mut to_index = Vec::new();
        let mut skipped = 0usize;

        for abs_path in files {
        let rel = pathdiff::diff_paths(abs_path, vault)
            .with_context(|| format!("failed to compute relative path for {}", abs_path))?
            .to_string_lossy()
            .to_string();

            if let Some(prev) = previous.files.get(&rel) {
                let current_mtime = Self::mtime_nanos(abs_path)?;
                if current_mtime == prev.mtime_ns {
                    skipped += 1;
                    continue;
                }
                let hash = Self::compute_hash(abs_path)?;
                if hash == prev.hash {
                    skipped += 1;
                    continue;
                }
            }

            to_index.push(abs_path.clone());
        }

        let current_abs: std::collections::HashSet<String> =
            files.iter().cloned().collect();
        let mut to_remove = Vec::new();

        for (rel, _) in &previous.files {
            let abs = vault.join(rel).to_string_lossy().to_string();
            if !current_abs.contains(&abs) {
                to_remove.push(abs);
            }
        }

        Ok((to_index, to_remove, skipped))
    }
}

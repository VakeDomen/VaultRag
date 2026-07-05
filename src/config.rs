use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub qdrant: QdrantConfig,
    pub embedding: EmbeddingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantConfig {
    pub host: String,
    pub grpc_port: u16,
    pub rest_port: u16,
    pub collection_name: String,
    pub docker_container_name: String,
    pub docker_volume_name: String,
    pub docker_image: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    pub model: String,
    pub dimension: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            qdrant: QdrantConfig {
                host: "localhost".to_string(),
                grpc_port: 6339,
                rest_port: 6338,
                collection_name: "vault_chunks".to_string(),
                docker_container_name: "vaultrag-qdrant".to_string(),
                docker_volume_name: "vaultrag_data".to_string(),
                docker_image: "qdrant/qdrant:latest".to_string(),
            },
            embedding: EmbeddingConfig {
                model: "all-MiniLM-L6-v2".to_string(),
                dimension: 384,
            },
        }
    }
}

impl Config {
    pub fn config_dir() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("could not find config directory")?
            .join("vaultrag");
        Ok(dir)
    }

    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read config at {}", path.display()))?;
            let config: Config = toml::from_str(&content)
                .with_context(|| format!("failed to parse config at {}", path.display()))?;
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir()?;
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create config dir {}", dir.display()))?;
        let path = Self::config_path()?;
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)
            .with_context(|| format!("failed to write config to {}", path.display()))?;
        Ok(())
    }
}

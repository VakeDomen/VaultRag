use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub vault: VaultConfig,
    pub qdrant: QdrantConfig,
    pub embedding: EmbeddingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    pub path: Option<String>,
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
            vault: VaultConfig { path: None },
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
            // Parse as generic table, merge with defaults for migration
            let mut raw: toml::Value = toml::from_str(&content)?;
            let default_raw: toml::Value =
                toml::from_str(&toml::to_string(&Config::default())?)?;
            merge_tables(&mut raw, &default_raw);
            let config: Config = raw.try_into()?;
            config.save()?;
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

    pub fn get(&self, key: &str) -> Result<String> {
        match key {
            "vault.path" => Ok(self.vault.path.clone().unwrap_or_default()),
            "qdrant.host" => Ok(self.qdrant.host.clone()),
            "qdrant.grpc_port" => Ok(self.qdrant.grpc_port.to_string()),
            "qdrant.rest_port" => Ok(self.qdrant.rest_port.to_string()),
            "qdrant.collection_name" => Ok(self.qdrant.collection_name.clone()),
            "qdrant.docker_container_name" => Ok(self.qdrant.docker_container_name.clone()),
            "qdrant.docker_volume_name" => Ok(self.qdrant.docker_volume_name.clone()),
            "qdrant.docker_image" => Ok(self.qdrant.docker_image.clone()),
            "embedding.model" => Ok(self.embedding.model.clone()),
            "embedding.dimension" => Ok(self.embedding.dimension.to_string()),
            _ => bail!("unknown config key: {key}"),
        }
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "vault.path" => self.vault.path = Some(value.to_string()),
            "qdrant.host" => self.qdrant.host = value.to_string(),
            "qdrant.grpc_port" => self.qdrant.grpc_port = value.parse()?,
            "qdrant.rest_port" => self.qdrant.rest_port = value.parse()?,
            "qdrant.collection_name" => self.qdrant.collection_name = value.to_string(),
            "qdrant.docker_container_name" => self.qdrant.docker_container_name = value.to_string(),
            "qdrant.docker_volume_name" => self.qdrant.docker_volume_name = value.to_string(),
            "qdrant.docker_image" => self.qdrant.docker_image = value.to_string(),
            "embedding.model" => self.embedding.model = value.to_string(),
            "embedding.dimension" => self.embedding.dimension = value.parse()?,
            _ => bail!("unknown config key: {key}"),
        }
        Ok(())
    }
}

fn merge_tables(target: &mut toml::Value, default: &toml::Value) {
    match (target, default) {
        (toml::Value::Table(t), toml::Value::Table(d)) => {
            for (key, val) in d {
                if !t.contains_key(key) {
                    t.insert(key.clone(), val.clone());
                } else {
                    merge_tables(&mut t[key], val);
                }
            }
        }
        _ => {}
    }
}

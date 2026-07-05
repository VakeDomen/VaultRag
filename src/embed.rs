use crate::config::Config;
use anyhow::{Context, Result};
use reagent_rs::{EmbeddingInvocationBuilder, InvocationBuilder};

fn build_embedder(config: &Config) -> EmbeddingInvocationBuilder {
    let mut builder = InvocationBuilder::embedding()
        .model(&config.embedding.model)
        .set_provider(parse_provider(&config.embedding.provider));

    if !config.embedding.base_url.is_empty() {
        builder = builder.set_base_url(&config.embedding.base_url);
    }
    if !config.embedding.api_key.is_empty() {
        builder = builder.set_api_key(&config.embedding.api_key);
    }

    builder
}

fn parse_provider(s: &str) -> reagent_rs::Provider {
    match s.to_lowercase().as_str() {
        "ollama" => reagent_rs::Provider::Ollama,
        "openai" => reagent_rs::Provider::OpenAi,
        "openrouter" => reagent_rs::Provider::OpenRouter,
        _ => reagent_rs::Provider::Ollama,
    }
}

pub struct Embedder;

impl Embedder {
    pub async fn embed(config: &Config, text: &str) -> Result<Vec<f32>> {
        let resp = build_embedder(config)
            .input(text)
            .invoke()
            .await
            .context("failed to embed text")?;

        Ok(resp.embedding.into_iter().map(|v| v as f32).collect())
    }

    pub async fn embed_batch(config: &Config, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let resp = build_embedder(config)
            .inputs(texts.to_vec())
            .invoke()
            .await
            .context("failed to embed batch")?;

        Ok(resp
            .embeddings
            .into_iter()
            .map(|vec| vec.into_iter().map(|v| v as f32).collect())
            .collect())
    }
}

use crate::config::Config;
use anyhow::{Context, Result};
use reagent_rs::{InvocationBuilder, Message};

pub struct HyPEGenerator;

impl HyPEGenerator {
    /// Generate hypothetical questions for a single chunk.
    pub async fn generate(config: &Config, text: &str) -> Result<Vec<String>> {
        let prompt = format!(
            "Analyze the passage below and generate essential questions that, \
             when answered, capture the main points and core meaning. \
             Questions should be exhaustive and understandable without context. \
             Named entities should be referenced by their full name.\n\n\
             Passage:\n{text}\n\n\
             Questions:"
        );

        let mut builder = InvocationBuilder::default()
            .model(&config.llm.model)
            .set_provider(parse_provider(&config.llm.provider))
            .add_message(Message::user(&prompt));

        if !config.llm.base_url.is_empty() {
            builder = builder.set_base_url(&config.llm.base_url);
        }
        if !config.llm.api_key.is_empty() {
            builder = builder.set_api_key(&config.llm.api_key);
        }

        let resp = builder
            .invoke()
            .await
            .context("failed to generate hypothetical questions")?;

        let content = resp
            .message
            .content
            .context("LLM returned empty response")?;

        let questions: Vec<String> = content
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(|l| {
                l.trim_start_matches(|c: char| {
                    c.is_ascii_digit() || c == '.' || c == ')' || c == ' '
                })
                .trim()
                .to_string()
            })
            .filter(|l| !l.is_empty())
            .collect();

        if questions.is_empty() {
            anyhow::bail!("LLM returned no questions for chunk");
        }

        Ok(questions)
    }
}

fn parse_provider(s: &str) -> reagent_rs::Provider {
    match s.to_lowercase().as_str() {
        "ollama" => reagent_rs::Provider::Ollama,
        "openai" => reagent_rs::Provider::OpenAi,
        "openrouter" => reagent_rs::Provider::OpenRouter,
        _ => reagent_rs::Provider::Ollama,
    }
}

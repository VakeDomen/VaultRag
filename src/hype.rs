use crate::config::Config;
use anyhow::{Context, Result};
use reagent_rs::{InvocationBuilder, Message};

pub struct HyPEGenerator;

impl HyPEGenerator {
    /// Generate hypothetical questions for a single chunk.
    pub async fn generate(config: &Config, text: &str) -> Result<Vec<String>> {
        let prompt = format!(
            "Analyze the passage below and generate standalone hypothetical user questions of which the answer can be found in the passage.

            The questions should be useful for retrieval in a RAG system. They should cover the passage exhaustively.

            Rules:

            * Each question must be understandable without seeing the passage.
            * Each question must be answerable from the passage.
            * Each question must include at least one concrete anchor from the passage.\
            * Valid anchors include full named entities, technical terms, dates, numbers, locations, product names, method names, events, decisions, requirements, constraints, causes, effects, or relationships.\
            * Use full names for all named entities.\
            * Avoid pronouns or vague references.\
            * Do not generate generic questions such as 'What is described in the passage?', 'What are the main points?', or 'What does the text say?'\
            * A question could apply to almost any passage, it is too generic so needs to be written with a concrete anchor.\
            * Include questions about key facts, definitions, relationships, causes, effects, requirements, constraints, comparisons, dates, numbers, and conclusions when present.\
            * Do not generate questions about minor wording details unless they are important to the meaning.\
            * Each question should be in own line, one line per question. no other delimiters \
            \
            \
             Passage:\n\n{text}"
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

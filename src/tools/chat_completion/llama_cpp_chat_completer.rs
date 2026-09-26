use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use futures::channel::mpsc;
use futures_util::Stream;
use llama_cpp_2::{
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{AddBos, LlamaChatMessage, LlamaModel, params::LlamaModelParams},
    sampling::LlamaSampler,
};
use rust_decimal::Decimal;
use std::{pin::Pin, sync::Arc};

use super::{ChatCompleter, ChatCompletionMessage, ChatCompletionResponse, Content, MessageRole};
use crate::prelude::TaskConfig;

#[derive(Clone, Debug)]
pub struct LlamaCppConfig {
    pub context_size: u32,
    pub max_tokens: usize,
    pub temperature: f32,
    pub top_k: i32,
    pub top_p: f32,
    pub seed: u32,
}

impl Default for LlamaCppConfig {
    fn default() -> Self {
        Self {
            context_size: 4096,
            max_tokens: 512,
            temperature: 0.7,
            top_k: 40,
            top_p: 0.95,
            seed: 0,
        }
    }
}

struct LlamaEngine {
    backend: LlamaBackend,
    model: LlamaModel,
}

#[derive(Clone)]
pub struct LlamaCppChatCompleter {
    engine: Arc<LlamaEngine>,
    config: LlamaCppConfig,
}

impl LlamaCppChatCompleter {
    pub fn from_file(path: impl AsRef<std::path::Path>, config: LlamaCppConfig) -> Result<Self> {
        let mut backend = LlamaBackend::init().context("initialize llama.cpp backend")?;
        backend.void_logs();
        let model = LlamaModel::load_from_file(&backend, path, &LlamaModelParams::default())
            .context("load GGUF model")?;
        Ok(Self {
            engine: Arc::new(LlamaEngine { backend, model }),
            config,
        })
    }

    pub fn config(&self) -> &LlamaCppConfig {
        &self.config
    }
}

fn role_name(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::user => "user",
        MessageRole::system => "system",
        MessageRole::assistant => "assistant",
        MessageRole::function => "tool",
    }
}

fn to_llama_messages(messages: &[ChatCompletionMessage]) -> Result<Vec<LlamaChatMessage>> {
    messages
        .iter()
        .map(|message| {
            let content = match &message.content {
                Content::Text(text) => text.clone(),
                Content::ImageUrl(_) => {
                    return Err(anyhow!(
                        "image messages are not supported by the local GGUF backend"
                    ));
                }
            };
            Ok(LlamaChatMessage::new(
                role_name(&message.role).into(),
                content,
            )?)
        })
        .collect()
}

fn generate(
    engine: &LlamaEngine,
    config: &LlamaCppConfig,
    messages: Vec<ChatCompletionMessage>,
    output: &mpsc::UnboundedSender<Result<ChatCompletionResponse>>,
) -> Result<()> {
    let chat = to_llama_messages(&messages)?;
    let template = engine
        .model
        .chat_template(None)
        .context("GGUF model has no chat template")?;
    let prompt = engine
        .model
        .apply_chat_template(&template, &chat, true)
        .context("apply GGUF chat template")?;
    let prompt_tokens = engine
        .model
        .str_to_token(&prompt, AddBos::Never)
        .map_err(|error| anyhow!("tokenize prompt: {error}"))?;
    if prompt_tokens.len() >= config.context_size as usize {
        return Err(anyhow!("local chat prompt exceeds the configured context window"));
    }
    let context_params = llama_cpp_2::context::params::LlamaContextParams::default()
        .with_n_ctx(std::num::NonZeroU32::new(config.context_size))
        .with_n_batch(config.context_size);
    let mut context = engine
        .model
        .new_context(&engine.backend, context_params)
        .context("create llama.cpp context")?;
    let mut batch = LlamaBatch::new(config.context_size as usize, 1);
    batch
        .add_sequence(&prompt_tokens, 0, false)
        .context("add prompt tokens")?;
    context.decode(&mut batch).context("decode prompt")?;

    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::top_k(config.top_k),
        LlamaSampler::top_p(config.top_p, 1),
        LlamaSampler::temp(config.temperature),
        LlamaSampler::dist(config.seed),
    ]);
    let mut position = prompt_tokens.len() as i32;
    let mut generated = 0usize;
    while generated < config.max_tokens && position < config.context_size as i32 {
        let token = sampler.sample(&context, -1);
        sampler.accept(token);
        if engine.model.is_eog_token(token) {
            break;
        }
        let piece = String::from_utf8_lossy(
            &engine
                .model
                .token_to_piece_bytes(token, 256, false, None)
                .map_err(|error| anyhow!("decode token: {error}"))?,
        )
        .into_owned();
        if !piece.is_empty() {
            output
                .unbounded_send(Ok(ChatCompletionResponse {
                    completion: piece,
                    estimated_cost: Decimal::ZERO,
                    usage_quantity: 1.0,
                    usage_unit: "tokens".into(),
                }))
                .map_err(|_| anyhow!("llama.cpp response stream was closed"))?;
        }
        batch.clear();
        batch
            .add(token, position, &[0], true)
            .context("add generated token")?;
        context
            .decode(&mut batch)
            .context("decode generated token")?;
        position += 1;
        generated += 1;
    }
    Ok(())
}

#[async_trait]
impl ChatCompleter for LlamaCppChatCompleter {
    async fn get_response(
        &mut self,
        messages: Vec<ChatCompletionMessage>,
        _task_configs: Vec<TaskConfig>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatCompletionResponse>> + Send>>> {
        let engine = Arc::clone(&self.engine);
        let config = self.config.clone();
        let (output, receiver) = mpsc::unbounded();
        let worker_output = output.clone();
        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                generate(&engine, &config, messages, &worker_output)
            })
            .await;
            if let Err(error) = result {
                let _ = output.unbounded_send(Err(anyhow!("join llama.cpp worker: {error}")));
            } else if let Ok(Err(error)) = result {
                let _ = output.unbounded_send(Err(error));
            }
        });
        Ok(Box::pin(receiver))
    }
}

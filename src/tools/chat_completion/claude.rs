use std::{pin::Pin, *};
use async_trait::async_trait;
use futures::Stream;
use anyhow::{Result, anyhow};
use crate::prelude::*;

use anthropic::client::Client;
use anthropic::config::AnthropicConfig;
use anthropic::types::{CreateMessageRequestBuilder, StopReason};
use anthropic::{AI_PROMPT, HUMAN_PROMPT};

use tokio_stream::StreamExt;

pub struct Claude {
    pub api_key: String
}

impl Claude {
    pub fn new_from_env() -> Self {
        Self { api_key: env::var("CLAUDE_API_KEY").unwrap() }
    }
}

#[async_trait]
impl ChatCompleter for Claude {
    async fn get_response(&self, messages: Vec<super::ChatCompletionMessage>, task_configs: Vec<TaskConfig>) -> Result<Pin<Box<dyn Stream<Item = Result<super::ChatCompletionResponse>> + Send>>> {
        // = futures_channel::channel();
        //eventsource_stream::

        let cfg = AnthropicConfig {
            api_key: self.api_key.clone(),
            ..Default::default()
        };

        let client = Client::try_from(cfg)?;

        let create_message_request = CreateMessageRequestBuilder::default()
        .model("claude-3".to_string())
        //.max_tokens_to_sample(256usize)
        .stream(true)
        .stop_sequences(vec![HUMAN_PROMPT.to_string()])
        .build()?;

        // Send a completion request.
        let mut stream = client.create_message_stream(create_message_request).await?;

        let stream = stream.map(|x| {
            match x {
                Ok(x) => {
                    Ok(super::ChatCompletionResponse {
                        completion: todo!()
                    })
                },
                Err(e) => {
                    Err(anyhow!(e))
                },
            }
        });
        Ok(Box::pin(stream))
    }
}
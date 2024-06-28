use std::{pin::Pin, *};
use async_trait::async_trait;
use futures::Stream;
use anyhow::{Result, anyhow};
use crate::prelude::*;

use anthropic::client::Client;
use anthropic::config::AnthropicConfig;
use anthropic::types::*;
use anthropic::{AI_PROMPT, HUMAN_PROMPT};

use tokio_stream::StreamExt;

#[derive(Clone)]
pub struct ClaudeChatCompleter {
    pub api_key: String
}

impl ClaudeChatCompleter {
    pub fn new_from_env() -> Self {
        Self { api_key: env::var("CLAUDE_API_KEY").unwrap() }
    }
}

#[async_trait]
impl ChatCompleter for ClaudeChatCompleter {
    async fn get_response(&mut self, messages: Vec<super::ChatCompletionMessage>, task_configs: Vec<TaskConfig>) -> Result<Pin<Box<dyn Stream<Item = Result<super::ChatCompletionResponse>> + Send>>> {
        // = futures_channel::channel();
        //eventsource_stream::

        let cfg = AnthropicConfig {
            api_key: self.api_key.clone(),
            ..Default::default()
        };

        let client = Client::try_from(cfg)?;

        let mut messages: Vec<Message> = messages.into_iter().map(|message| {

            dbg!(message.clone());
            Message {
                role: match message.role {
                    chat_completion::MessageRole::user => anthropic::types::Role::User,
                    chat_completion::MessageRole::system => anthropic::types::Role::User,
                    chat_completion::MessageRole::assistant => anthropic::types::Role::Assistant,
                    chat_completion::MessageRole::function => anthropic::types::Role::Assistant,
                },
                content: match message.content {
                    super::Content::Text(text) => vec![ContentBlock::Text { text: text }],
                    // TODO: Requires changes to work
                    super::Content::ImageUrl(image_url) => vec![ContentBlock::Image { source: "base64".to_string(), media_type: "image/jpeg".to_string(), data: todo!() }]
                }
            }
        }).collect();

        let message_request = MessagesRequestBuilder::default()
        .model("claude-3-5-sonnet".to_string())
        //.max_tokens_to_sample(256usize)
        .stream(true).messages(messages)
        .stop_sequences(vec![HUMAN_PROMPT.to_string()])
        .build()?;

        // Send a completion request.
        let mut stream = client.messages_stream(message_request).await?;

        let stream = stream.map(|x| {
            match x {
                Ok(x) => {
                    let completion_response = match x {
                        MessagesStreamEvent::MessageStart { message } => {
                            "".to_string()
                        },
                        MessagesStreamEvent::ContentBlockStart { index, content_block } => {
                            "".to_string()
                        },
                        MessagesStreamEvent::ContentBlockDelta { index, delta } => {
                            match delta {
                                ContentBlockDelta::TextDelta { text } => {
                                    text
                                },
                            }
                        },
                        MessagesStreamEvent::ContentBlockStop { index } => {
                            "".to_string()
                        },
                        MessagesStreamEvent::MessageDelta { delta, usage } => {
                            "".to_string()
                        },
                        MessagesStreamEvent::MessageStop => {
                            "".to_string()
                        },
                    };
                    Ok(super::ChatCompletionResponse {
                        completion: completion_response
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
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "xtensa"))]
pub mod claude_chat_completer;
use std::pin::Pin;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "xtensa"))]
pub use claude_chat_completer::*;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "xtensa"))]
pub mod chat_gpt_chat_completer;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "xtensa"))]
pub use chat_gpt_chat_completer::*;

pub mod candle_chat_completer;
pub use candle_chat_completer::*;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::prelude::*;
use futures_util::{Stream, FutureExt, StreamExt, stream, TryStreamExt};
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[allow(non_camel_case_types)]
pub enum MessageRole {
    user,
    system,
    assistant,
    function,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatCompletionMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCall>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Function {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parameters: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChatCompletionResponse {
    pub completion: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChatCompletionChoice {
    pub index: i64,
    pub message: Option<ChatCompletionMessageForResponse>,
    //pub finish_reason: Option<FinishReason>,
    pub delta: ChatCompletionMessageForResponse
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatCompletionMessageForResponse {
    pub role: Option<MessageRole>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCall>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionParameters {
}

#[async_trait]
pub trait ChatCompleter: Send + Sync {
    async fn get_response(&self, messages: Vec<ChatCompletionMessage>, task_configs: Vec<TaskConfig>) -> Result<Pin<Box<dyn Stream<Item = Result<ChatCompletionResponse>> + Send>>>;
}

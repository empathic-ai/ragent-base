use crate::prelude::*;
use std::{pin::Pin, *};
use futures::Stream;
use futures_util::{StreamExt, stream};
use anyhow::*;
use async_trait::async_trait;

pub struct Llama {
}

impl Llama {
    pub fn new() -> Self {
        Self { }
    }
}

#[async_trait]
impl ChatCompleter for Llama {
    async fn get_response(&self, messages: Vec<super::ChatCompletionMessage>, task_configs: Vec<TaskConfig>) -> Result<Pin<Box<dyn Stream<Item = Result<super::ChatCompletionResponse>> + Send>>> {
        todo!();
    }
}

/* 
impl Agent for Llama {
    fn new_message(&mut self, role: Role, text: String) {
 
    }

    async fn get_response(&self, prompt: String, token: CancellationToken) -> Result<impl Stream<Item = Result<LLMResponse>>> {
        let items = vec![Ok(LLMResponse::new("".to_string()))];
        let mut stream = stream::iter(items);
        Ok(stream)
    }

    fn new(config: AgentConfig) -> Self {
        Llama {
        }
    }

    fn get_config(&mut self) -> AgentConfig {
        todo!()
    }

    async fn send_event(&mut self, task: UserEvent) {
        todo!()
    }
}
*/
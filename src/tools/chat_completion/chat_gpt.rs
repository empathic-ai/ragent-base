use async_channel::{Sender, Receiver};
use bytes::Bytes;
use futures_util::lock::Mutex;
use openai_api_rs::v1::api::Client;
use openai_api_rs::v1::chat_completion::*;
use openai_api_rs::v1::fine_tune::ModelDetails;

use async_trait::async_trait;

use uuid::Uuid;
use crate::prelude::*;
use std::{pin::Pin, *};
use std::collections::HashMap;
use std::sync::Arc;
use futures::Stream;
use futures_util::StreamExt;
use anyhow::{Result, anyhow};
use common::prelude::*;
use empathic_audio::*;

pub struct ChatGPT {
    pub api_key: String
}

impl ChatGPT {
    pub fn new_from_env() -> Self {
        Self { api_key: env::var("OPENAI_API_KEY").unwrap() }
    }
}

#[async_trait]
impl ChatCompleter for ChatGPT {
    async fn get_response(&self, messages: Vec<super::ChatCompletionMessage>, task_configs: Vec<TaskConfig>) -> Result<Pin<Box<dyn Stream<Item = Result<super::ChatCompletionResponse>> + Send>>> {
        //GPT3_5_TURBO
        //GPT4_0613

        let mut messages: Vec<_> = messages.into_iter().map(|x| {
            openai_api_rs::v1::chat_completion::ChatCompletionMessage {
                role: todo!(),
                content: todo!(),
                name: todo!(),
                function_call: todo!()
            }
        }).collect();

        let mut functions = Vec::<openai_api_rs::v1::chat_completion::Function>::new();

        let model_name = GPT4_0613.to_string();// GPT4_0613.to_string();

        // TODO: Uncomment and use is_function_model() if built-in functions are preferable
        let is_function_model = false;//Self::is_function_model(model_name.clone());

        if !is_function_model {
            let function_prompt = super::get_function_prompt(task_configs.clone());
            messages.insert(0,  openai_api_rs::v1::chat_completion::ChatCompletionMessage {
                role:  openai_api_rs::v1::chat_completion::MessageRole::system,
                content: function_prompt,
                name: None,
                function_call: None
            });
        };

        if is_function_model {

            for config in task_configs {
                let mut properties = HashMap::<String, Box<JSONSchemaDefine>>::new();

                for parameter in config.parameters {
                    properties.insert(parameter.name, Box::new(JSONSchemaDefine {
                        schema_type: Some(JSONSchemaType::String),
                        description: Some(parameter.description),
                        enum_values: None,
                        properties: None,
                        required: None,
                        items: None,
                    }));
                }

                let required = properties.keys().clone().map(|x| x.to_owned()).collect();
                functions.push(openai_api_rs::v1::chat_completion::Function {
                    name: config.name.clone(),
                    description: Some(config.description.clone()),
                    parameters: openai_api_rs::v1::chat_completion::FunctionParameters {
                        schema_type: JSONSchemaType::Object,
                        properties: Some(properties),
                        required: Some(required),
                    }
                });
            }
        }

        let chat_completion_request: ChatCompletionRequest = ChatCompletionRequest::new(model_name.clone(), messages).stream(true);
       
        let client = Client::new(self.api_key.clone());
        let mut stream = client.chat_completion_stream(chat_completion_request.clone()).await.expect("Failed to get chat completion stream.");
        
        let stream = stream.map(|x| {
            match x {
                Ok(x) => {
                    Ok(super::ChatCompletionResponse {
                        completion: x.choices[0].delta.content.clone().unwrap()
                    })
                },
                Err(e) => {
                    Err(e)
                },
            }
        });
        Ok(Box::pin(stream))
    }
}

fn is_function_model(mode_name: String) -> bool {
    match mode_name.as_str() {
        GPT4_1106_PREVIEW => true,
        _ => false
    }
}
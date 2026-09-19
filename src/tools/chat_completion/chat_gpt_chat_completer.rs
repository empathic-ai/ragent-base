use async_channel::{Sender, Receiver};
use bytes::Bytes;
use futures_util::lock::Mutex;
use tracing::info;
use openai_api_rs::v1::api::{OpenAIClient, OpenAIClientBuilder};
use openai_api_rs::v1::chat_completion::chat_completion::ChatCompletionRequest;
use openai_api_rs::v1::chat_completion::chat_completion_stream::{
    ChatCompletionStreamRequest,
    ChatCompletionStreamResponse,
    StreamOptions,
};
use openai_api_rs::v1::{types::*, chat_completion::*, common::*};

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
use delune::*;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;

type ChatGPTContent = openai_api_rs::v1::chat_completion::Content;

#[derive(Clone)]
pub struct ChatGPTChatCompleter {
    pub api_key: String
}

impl ChatGPTChatCompleter {
    pub fn new_from_env() -> Self {
        Self { api_key: env::var("OPENAI_API_KEY").unwrap() }
    }
}

#[async_trait]
impl ChatCompleter for ChatGPTChatCompleter {
    async fn get_response(&mut self, messages: Vec<super::ChatCompletionMessage>, task_configs: Vec<TaskConfig>) -> Result<Pin<Box<dyn Stream<Item = Result<super::ChatCompletionResponse>> + Send>>> {
        //GPT3_5_TURBO
        //GPT4_0613

        info!("Getting chat response from OpenAI API...");

        let mut messages: Vec<_> = messages.into_iter().map(|message| {

            //dbg!(message.clone());
            openai_api_rs::v1::chat_completion::ChatCompletionMessage {
                role: match message.role {
                    super::MessageRole::user =>  openai_api_rs::v1::chat_completion::MessageRole::user,
                    super::MessageRole::system =>  openai_api_rs::v1::chat_completion::MessageRole::system,
                    super::MessageRole::assistant =>  openai_api_rs::v1::chat_completion::MessageRole::assistant,
                    super::MessageRole::function =>  openai_api_rs::v1::chat_completion::MessageRole::function,
                },
                content: match message.content {
                    super::Content::Text(text) => openai_api_rs::v1::chat_completion::Content::Text(text),
                    super::Content::ImageUrl(image_url) => openai_api_rs::v1::chat_completion::Content::ImageUrl(
                        vec![
                            openai_api_rs::v1::chat_completion::ImageUrl {
                                r#type: openai_api_rs::v1::chat_completion::ContentType::image_url,
                                text: None,
                                image_url: Some(openai_api_rs::v1::chat_completion::ImageUrlType { url: image_url[0].image_url.clone().unwrap().url })
                            }
                        ]
                    ),
                },
                name: message.name,
                tool_calls: None,
                tool_call_id: None
            }
        }).collect();

        let mut functions = Vec::<openai_api_rs::v1::types::Function>::new();

        let model = GPT4_O;// GPT4_0613.to_string();

        // TODO: Uncomment and use is_function_model() if built-in functions are preferable
        let is_function_model = false;//Self::is_function_model(model_name.clone());

        if !is_function_model && !task_configs.is_empty() {
            let function_prompt = super::get_function_prompt(task_configs.clone());
            messages.insert(0,  openai_api_rs::v1::chat_completion::ChatCompletionMessage {
                role:  openai_api_rs::v1::chat_completion::MessageRole::system,
                content: openai_api_rs::v1::chat_completion::Content::Text(function_prompt),
                name: None,
                tool_calls: None,
                tool_call_id: None
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
                functions.push(openai_api_rs::v1::types::Function {
                    name: config.name.clone(),
                    description: Some(config.description.clone()),
                    parameters: openai_api_rs::v1::types::FunctionParameters {
                        schema_type: JSONSchemaType::Object,
                        properties: Some(properties),
                        required: Some(required),
                    }
                });
            }
        }

        let chat_completion_request =
            ChatCompletionStreamRequest::new(model.to_string(), messages)
                .stream_options(StreamOptions {
                    include_usage: true,
                });

        let client = OpenAIClientBuilder::new().with_api_key(self.api_key.clone()).build().unwrap();
        let mut stream = client.chat_completion_stream(chat_completion_request.clone()).await.map_err(|x| anyhow!("Failed to get chat completion stream: {}", x))?;
        
        let model = model.to_string();
        let stream = stream.map(move |x| {
            match x {
                ChatCompletionStreamResponse::Content(content) => {
                    Ok(super::ChatCompletionResponse {
                        completion: content,
                        estimated_cost: Decimal::ZERO,
                    })
                }

                ChatCompletionStreamResponse::Usage(usage) => {
                    let cost = calculate_cost(&model, &usage)?;

                    // Emit the cost exactly once, at the end of the stream.
                    Ok(super::ChatCompletionResponse {
                        completion: String::new(),
                        estimated_cost: cost,
                    })
                }

                ChatCompletionStreamResponse::Done => {
                    Ok(super::ChatCompletionResponse {
                        completion: String::new(),
                        estimated_cost: Decimal::ZERO,
                    })
                }

                _ => {
                    Ok(super::ChatCompletionResponse {
                        completion: String::new(),
                        estimated_cost: Decimal::ZERO,
                    })
                }
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

fn calculate_cost(
    model: &str,
    usage: &openai_api_rs::v1::common::Usage,
) -> Result<Decimal> {
    let cached_tokens = usage
        .prompt_tokens_details
        .as_ref()
        .map(|details| details.cached_tokens)
        .unwrap_or(0);

    let uncached_tokens = usage.prompt_tokens - cached_tokens;

    let (input_price, cached_input_price, output_price) = match model {
        GPT4_O
        | GPT4_O_2024_08_06
        | GPT4_O_2024_11_20 => (
            dec!(2.50),
            dec!(1.25),
            dec!(10.00),
        ),

        GPT4_O_2024_05_13 => (
            dec!(5.00),
            dec!(5.00),
            dec!(15.00),
        ),

        GPT4_O_MINI
        | GPT4_O_MINI_2024_07_18 => (
            dec!(0.15),
            dec!(0.075),
            dec!(0.60),
        ),

        GPT4_1
        | GPT4_1_2025_04_14 => (
            dec!(2.00),
            dec!(0.50),
            dec!(8.00),
        ),

        GPT4_1_MINI
        | GPT4_1_MINI_2025_04_14 => (
            dec!(0.40),
            dec!(0.10),
            dec!(1.60),
        ),

        GPT4_1_NANO
        | GPT4_1_NANO_2025_04_14 => (
            dec!(0.10),
            dec!(0.025),
            dec!(0.40),
        ),

        GPT5
        | GPT5_2025_08_07
        | GPT5_CHAT_LATEST
        | GPT5_CODEX => (
            dec!(1.25),
            dec!(0.125),
            dec!(10.00),
        ),

        GPT5_MINI
        | GPT5_MINI_2025_08_07 => (
            dec!(0.25),
            dec!(0.025),
            dec!(2.00),
        ),

        GPT5_NANO
        | GPT5_NANO_2025_08_07 => (
            dec!(0.05),
            dec!(0.005),
            dec!(0.40),
        ),

        GPT5_PRO
        | GPT5_PRO_2025_10_06 => (
            dec!(15.00),
            dec!(15.00),
            dec!(120.00),
        ),

        O1
        | O1_2024_12_17 => (
            dec!(15.00),
            dec!(7.50),
            dec!(60.00),
        ),

        O1_PRO
        | O1_PRO_2025_03_19 => (
            dec!(150.00),
            dec!(150.00),
            dec!(600.00),
        ),

        O3
        | O3_2025_04_16 => (
            dec!(2.00),
            dec!(0.50),
            dec!(8.00),
        ),

        O3_MINI
        | O3_MINI_2025_01_31 => (
            dec!(1.10),
            dec!(0.55),
            dec!(4.40),
        ),

        O4_MINI
        | O4_MINI_2025_04_16 => (
            dec!(1.10),
            dec!(0.275),
            dec!(4.40),
        ),

        model => {
            return Err(anyhow!(
                "No pricing configured for OpenAI model `{model}`"
            ));
        }
    };

    let million = dec!(1_000_000);

    let uncached_input_cost =
        Decimal::from(uncached_tokens) / million * input_price;

    let cached_input_cost =
        Decimal::from(cached_tokens) / million * cached_input_price;

    let output_cost =
        Decimal::from(usage.completion_tokens) / million * output_price;

    Ok(
        uncached_input_cost
            + cached_input_cost
            + output_cost
    )
}
use crate::prelude::*;
use std::{pin::Pin, *};
use futures::Stream;
use futures_util::{StreamExt, stream};
use anyhow::*;
use async_trait::async_trait;

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::mixformer::{Config, MixFormerSequentialForCausalLM as MixFormer};
use candle_transformers::models::quantized_mixformer::MixFormerSequentialForCausalLM as QMixFormer;
//use candle_wasm_example_phi::console_log;
//use js_sys::Date;
use serde::Deserialize;
use tokenizers::Tokenizer;
use std::result::Result::Ok;
//use wasm_bindgen::prelude::*;

enum SelectedModel {
    MixFormer(MixFormer),
    Quantized(QMixFormer),
}

pub struct Model {
    model: SelectedModel,
    tokenizer: Tokenizer,
    logits_processor: LogitsProcessor,
    tokens: Vec<u32>,
    repeat_penalty: f32,
    repeat_last_n: usize,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]

pub struct ModelName {
    pub _name_or_path: String,
}

impl Model {

    pub fn load(
        weights: Vec<u8>,
        tokenizer: Vec<u8>,
        config: Vec<u8>,
        quantized: bool,
    ) -> Result<Model> {
        //console_error_panic_hook::set_once();
        //console_log!("loading model");
        let device = Device::Cpu;
        let name: ModelName = serde_json::from_slice(&config)?;
        let config: Config = serde_json::from_slice(&config)?;

        //console_log!("config loaded {:?}", name);
        let tokenizer =
            Tokenizer::from_bytes(&tokenizer).map_err(|m| anyhow!(m.to_string()))?;
        //let start = Date::now();
        //console_log!("weights len: {:?}", weights.len());
        let model = if quantized {
            let vb = candle_transformers::quantized_var_builder::VarBuilder::from_gguf_buffer(
                &weights, &device,
            )?;
            //console_log!("weights loaded");
            if name._name_or_path == "microsoft/phi-2" {
                let model = QMixFormer::new_v2(&config, vb)?;
                SelectedModel::Quantized(model)
            } else {
                let model = QMixFormer::new(&config, vb)?;
                SelectedModel::Quantized(model)
            }
        } else {
            let device = &Device::Cpu;
            let vb = VarBuilder::from_buffered_safetensors(weights, DType::F32, device)?;
            let model = MixFormer::new(&config, vb)?;
            SelectedModel::MixFormer(model)
        };
        //console_log!("model loaded in {:?}s", (Date::now() - start) / 1000.);
        let logits_processor = LogitsProcessor::new(299792458, None, None);
        Ok(Self {
            model,
            tokenizer,
            tokens: vec![],
            logits_processor,
            repeat_penalty: 1.,
            repeat_last_n: 64,
        })
    }

    pub fn init_with_prompt(
        &mut self,
        prompt: String,
        temp: f64,
        top_p: f64,
        repeat_penalty: f32,
        repeat_last_n: usize,
        seed: u64,
    ) -> Result<String> {
        match &mut self.model {
            SelectedModel::MixFormer(m) => m.clear_kv_cache(),
            SelectedModel::Quantized(m) => m.clear_kv_cache(),
        };
        let temp = if temp <= 0. { None } else { Some(temp) };
        let top_p = if top_p <= 0. || top_p >= 1. {
            None
        } else {
            Some(top_p)
        };
        self.logits_processor = LogitsProcessor::new(seed, temp, top_p);
        self.repeat_penalty = repeat_penalty;
        self.repeat_last_n = repeat_last_n;
        self.tokens.clear();
        let tokens = self
            .tokenizer
            .encode(prompt, true)
            .map_err(anyhow::Error::msg)?
            .get_ids()
            .to_vec();
        let text = self
            .process(&tokens)?;
        Ok(text)
    }
    //#[wasm_bindgen]
    pub fn next_token(&mut self) -> Result<String> {
        let last_token = *self.tokens.last().unwrap();
        let text = self
            .process(&[last_token])?;
        Ok(text)
    }
}

impl Model {
    fn process(&mut self, tokens: &[u32]) -> candle_chat_completer::Result<String> {
        let dev = Device::Cpu;
        let input = Tensor::new(tokens, &dev)?.unsqueeze(0)?;
        let logits = match &mut self.model {
            SelectedModel::MixFormer(m) => m.forward(&input)?,
            SelectedModel::Quantized(m) => m.forward(&input)?,
        };
        let logits = logits.squeeze(0)?.to_dtype(DType::F32)?;
        let logits = if self.repeat_penalty == 1. {
            logits
        } else {
            let start_at = tokens.len().saturating_sub(self.repeat_last_n);
            candle_transformers::utils::apply_repeat_penalty(
                &logits,
                self.repeat_penalty,
                &tokens[start_at..],
            )?
        };

        let next_token = self.logits_processor.sample(&logits)?;
        self.tokens.push(next_token);
        let token = match self.tokenizer.decode(&[next_token], false) {
            Ok(token) => token,
            Err(e) => {
                //console_log!("error decoding token: {:?}", e);
                "".to_string()
            }
        };
        // console_log!("token: {:?}: {:?}", token, next_token);
        Ok(token)
    }
}

pub struct CandleChatCompleter {
}

impl CandleChatCompleter {
    pub fn new() -> Self {
        Self { }
    }
}

#[async_trait]
impl ChatCompleter for CandleChatCompleter {
    async fn get_response(&self, messages: Vec<super::ChatCompletionMessage>, task_configs: Vec<TaskConfig>) -> Result<Pin<Box<dyn Stream<Item = Result<super::ChatCompletionResponse>> + Send>>> {

        todo!();
    }
}
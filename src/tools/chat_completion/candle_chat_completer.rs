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

use candle_transformers::models::phi::{Config as PhiConfig, Model as Phi};
use candle_transformers::models::phi3::{Config as Phi3Config, Model as Phi3};
#[cfg(not(target_arch = "wasm32"))]
use hf_hub::api::sync::Api;
use hf_hub::Repo;
use hf_hub::RepoType;
//use candle_wasm_example_phi::console_log;
//use js_sys::Date;
use serde::Deserialize;
use tokenizers::Tokenizer;
use std::result::Result::Ok;

use candle_helpers::token_output_stream::TokenOutputStream;
//use wasm_bindgen::prelude::*;
use futures::Future;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WhichModel {
    V1,
    V1_5,
    V2,
    V3,
    V3Medium,
    V2Old,
    PuffinPhiV2,
    PhiHermes,
}

enum SelectedModel {
    MixFormer(MixFormer),
    #[cfg(not(target_arch = "wasm32"))]
    Phi(Phi),
    #[cfg(not(target_arch = "wasm32"))]
    Phi3(Phi3),
    Quantized(QMixFormer),
}

pub struct Model {
    model: SelectedModel,
    tokenizer: TokenOutputStream,
    //tokenizer: Tokenizer,
    #[cfg(not(target_arch = "wasm32"))]
    device: Device,
    logits_processor: LogitsProcessor,
    #[cfg(target_arch = "wasm32")]
    tokens: Vec<u32>,
    repeat_penalty: f32,
    repeat_last_n: usize,
    #[cfg(not(target_arch = "wasm32"))]
    verbose_prompt: bool,
}


#[derive(Debug, Clone, PartialEq, Deserialize)]

pub struct ModelName {
    pub _name_or_path: String,
}

#[derive(Debug)]
struct Args {
    /// Run on CPU rather than on GPU.
    cpu: bool,
    /// Enable tracing (generates a trace-timestamp.json file).
    tracing: bool,
    /// Display the token for the specified prompt.
    verbose_prompt: bool,
    mmlu_dir: Option<String>,
    /// The temperature used to generate samples.
    temperature: Option<f64>,
    /// Nucleus sampling probability cutoff.
    top_p: Option<f64>,
    /// The seed to use when generating random samples.
    seed: u64,
    model_id: Option<String>,
    model: WhichModel,
    revision: Option<String>,
    weight_file: Option<String>,
    tokenizer: Option<String>,
    quantized: bool,
    /// Penalty to be applied for repeating tokens, 1. means no penalty.
    repeat_penalty: f32,
    /// The context size to consider for the repeat penalty.
    repeat_last_n: usize,
    /// The dtype to be used for running the model, e.g. f32, bf16, or f16.
    dtype: Option<String>,
}

impl Model {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(args: Args) -> Result<Self> {
  
        println!(
            "avx: {}, neon: {}, simd128: {}, f16c: {}",
            candle_core::utils::with_avx(),
            candle_core::utils::with_neon(),
            candle_core::utils::with_simd128(),
            candle_core::utils::with_f16c()
        );
        println!(
            "temp: {:.2} repeat-penalty: {:.2} repeat-last-n: {}",
            args.temperature.unwrap_or(0.),
            args.repeat_penalty,
            args.repeat_last_n
        );
    
        let start = std::time::Instant::now();
        let api = Api::new()?;
        let model_id = match args.model_id {
            Some(model_id) => model_id.to_string(),
            None => {
                if args.quantized {
                    "lmz/candle-quantized-phi".to_string()
                } else {
                    match args.model {
                        WhichModel::V1 => "microsoft/phi-1".to_string(),
                        WhichModel::V1_5 => "microsoft/phi-1_5".to_string(),
                        WhichModel::V2 | WhichModel::V2Old => "microsoft/phi-2".to_string(),
                        WhichModel::V3 => "microsoft/Phi-3-mini-4k-instruct".to_string(),
                        WhichModel::V3Medium => "microsoft/Phi-3-medium-4k-instruct".to_string(),
                        WhichModel::PuffinPhiV2 | WhichModel::PhiHermes => {
                            "lmz/candle-quantized-phi".to_string()
                        }
                    }
                }
            }
        };
        let revision = match args.revision {
            Some(rev) => rev.to_string(),
            None => {
                if args.quantized {
                    "main".to_string()
                } else {
                    match args.model {
                        WhichModel::V1 => "refs/pr/8".to_string(),
                        WhichModel::V1_5 => "refs/pr/73".to_string(),
                        WhichModel::V2Old => "834565c23f9b28b96ccbeabe614dd906b6db551a".to_string(),
                        WhichModel::V2
                        | WhichModel::V3
                        | WhichModel::V3Medium
                        | WhichModel::PuffinPhiV2
                        | WhichModel::PhiHermes => "main".to_string(),
                    }
                }
            }
        };
        let repo = api.repo(Repo::with_revision(model_id, RepoType::Model, revision));
        let tokenizer_filename = match args.tokenizer {
            Some(file) => std::path::PathBuf::from(file),
            None => match args.model {
                WhichModel::V1
                | WhichModel::V1_5
                | WhichModel::V2
                | WhichModel::V2Old
                | WhichModel::V3
                | WhichModel::V3Medium => repo.get("tokenizer.json")?,
                WhichModel::PuffinPhiV2 | WhichModel::PhiHermes => {
                    repo.get("tokenizer-puffin-phi-v2.json")?
                }
            },
        };
        let filenames = match args.weight_file {
            Some(weight_file) => vec![std::path::PathBuf::from(weight_file)],
            None => {
                if args.quantized {
                    match args.model {
                        WhichModel::V1 => vec![repo.get("model-v1-q4k.gguf")?],
                        WhichModel::V1_5 => vec![repo.get("model-q4k.gguf")?],
                        WhichModel::V2 | WhichModel::V2Old => vec![repo.get("model-v2-q4k.gguf")?],
                        WhichModel::PuffinPhiV2 => vec![repo.get("model-puffin-phi-v2-q4k.gguf")?],
                        WhichModel::PhiHermes => vec![repo.get("model-phi-hermes-1_3B-q4k.gguf")?],
                        WhichModel::V3 | WhichModel::V3Medium => anyhow::bail!(
                            "use the quantized or quantized-phi examples for quantized phi-v3"
                        ),
                    }
                } else {
                    match args.model {
                        WhichModel::V1 | WhichModel::V1_5 => vec![repo.get("model.safetensors")?],
                        WhichModel::V2 | WhichModel::V2Old | WhichModel::V3 | WhichModel::V3Medium => {
                            candle_helpers::hub_load_safetensors(
                                &repo,
                                "model.safetensors.index.json",
                            )?
                        }
                        WhichModel::PuffinPhiV2 => vec![repo.get("model-puffin-phi-v2.safetensors")?],
                        WhichModel::PhiHermes => vec![repo.get("model-phi-hermes-1_3B.safetensors")?],
                    }
                }
            }
        };
        println!("retrieved the files in {:?}", start.elapsed());
        let tokenizer = Tokenizer::from_file(tokenizer_filename).map_err(anyhow::Error::msg)?;
    
        let start = std::time::Instant::now();
        let config = || match args.model {
            WhichModel::V1 => Config::v1(),
            WhichModel::V1_5 => Config::v1_5(),
            WhichModel::V2 | WhichModel::V2Old => Config::v2(),
            WhichModel::PuffinPhiV2 => Config::puffin_phi_v2(),
            WhichModel::PhiHermes => Config::phi_hermes_1_3b(),
            WhichModel::V3 | WhichModel::V3Medium => {
                panic!("use the quantized or quantized-phi examples for quantized phi-v3")
            }
        };
        let device = candle_helpers::device(args.cpu)?;
        let model = if args.quantized {
            let config = config();
            let vb = candle_transformers::quantized_var_builder::VarBuilder::from_gguf(
                &filenames[0],
                &device,
            )?;
            let model = match args.model {
                WhichModel::V2 | WhichModel::V2Old => QMixFormer::new_v2(&config, vb)?,
                _ => QMixFormer::new(&config, vb)?,
            };
            SelectedModel::Quantized(model)
        } else {
            let dtype = match args.dtype {
                Some(dtype) => std::str::FromStr::from_str(&dtype)?,
                None => {
                    if (args.model == WhichModel::V3 || args.model == WhichModel::V3Medium)
                        && device.is_cuda()
                    {
                        DType::BF16
                    } else {
                        DType::F32
                    }
                }
            };
            let vb = unsafe { VarBuilder::from_mmaped_safetensors(&filenames, dtype, &device)? };
            match args.model {
                WhichModel::V1 | WhichModel::V1_5 | WhichModel::V2 => {
                    let config_filename = repo.get("config.json")?;
                    let config = std::fs::read_to_string(config_filename)?;
                    let config: PhiConfig = serde_json::from_str(&config)?;
                    let phi = Phi::new(&config, vb)?;
                    SelectedModel::Phi(phi)
                }
                WhichModel::V3 | WhichModel::V3Medium => {
                    let config_filename = repo.get("config.json")?;
                    let config = std::fs::read_to_string(config_filename)?;
                    let config: Phi3Config = serde_json::from_str(&config)?;
                    let phi3 = Phi3::new(&config, vb)?;
                    SelectedModel::Phi3(phi3)
                }
                WhichModel::V2Old => {
                    let config = config();
                    SelectedModel::MixFormer(MixFormer::new_v2(&config, vb)?)
                }
                WhichModel::PhiHermes | WhichModel::PuffinPhiV2 => {
                    let config = config();
                    SelectedModel::MixFormer(MixFormer::new(&config, vb)?)
                }
            }
        };
        println!("loaded the model in {:?}", start.elapsed());

        let logits_processor = LogitsProcessor::new(args.seed, args.temperature, args.top_p);
        Ok(Self {
            model,
            tokenizer: TokenOutputStream::new(tokenizer),
            logits_processor,
            repeat_penalty: args.repeat_penalty,
            repeat_last_n: args.repeat_last_n,
            verbose_prompt: args.verbose_prompt,
            device: device.clone(),
        })
    }

    #[cfg(target_arch = "wasm32")]
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
            tokenizer: TokenOutputStream::new(tokenizer),
            tokens: vec![],
            logits_processor,
            repeat_penalty: 1.,
            repeat_last_n: 64,
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn run(mut self, prompt: String, sample_len: usize) -> impl Stream<Item = anyhow::Result<String>> {
        async_stream::try_stream! {
                
            //use std::io::Write;

            use candle_core::IndexOp;
            println!("starting the inference loop");
            let tokens = self
                .tokenizer
                .tokenizer()
                .encode(prompt.clone(), true)
                .map_err(anyhow::Error::msg)?;
            if tokens.is_empty() {
                Err(anyhow!("Empty prompts are not supported in the phi model."))?;
            }
            if self.verbose_prompt {
                for (token, id) in tokens.get_tokens().iter().zip(tokens.get_ids().iter()) {
                    let token = token.replace('▁', " ").replace("<0x0A>", "\n");
                    println!("{id:7} -> '{token}'");
                }
            }
            let mut tokens = tokens.get_ids().to_vec();
            let mut generated_tokens = 0usize;
            let eos_token = match self.tokenizer.get_token("<|endoftext|>") {
                Some(token) => token,
                None => Err(anyhow!("cannot find the endoftext token"))?,
            };
            println!("{prompt}");
            //std::io::stdout().flush()?;
            let start_gen = std::time::Instant::now();
            let mut pos = 0;
            for index in 0..sample_len {
                let context_size = if index > 0 { 1 } else { tokens.len() };
                let ctxt = &tokens[tokens.len().saturating_sub(context_size)..];
                let input = Tensor::new(ctxt, &self.device)?.unsqueeze(0)?;
                let logits = match &mut self.model {
                    SelectedModel::MixFormer(m) => m.forward(&input)?,
                    SelectedModel::Phi(m) => m.forward(&input)?,
                    SelectedModel::Quantized(m) => m.forward(&input)?,
                    SelectedModel::Phi3(m) => m.forward(&input, pos)?.i((.., 0, ..))?,
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
                tokens.push(next_token);
                generated_tokens += 1;
                if next_token == eos_token {
                    break;
                }
                if let Some(t) = self.tokenizer.next_token(next_token)? {
                    yield t;
                    //std::io::stdout().flush()?;
                }
                pos += context_size;
            }
            let dt = start_gen.elapsed();
            println!(
                "\n{generated_tokens} tokens generated ({:.2} token/s)",
                generated_tokens as f64 / dt.as_secs_f64(),
            );
        }
    }

    #[cfg(target_arch = "wasm32")]
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
            SelectedModel::Quantized(m) => m.clear_kv_cache()
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
            .tokenizer.tokenizer()
            .encode(prompt, true)
            .map_err(anyhow::Error::msg)?
            .get_ids()
            .to_vec();
        let text = self
            .process(&tokens)?;
        Ok(text)
    }
    //#[wasm_bindgen]

    #[cfg(target_arch = "wasm32")]
    pub fn next_token(&mut self) -> Result<String> {
        let last_token = *self.tokens.last().unwrap();
        let text = self
            .process(&[last_token])?;
        Ok(text)
    }
}

impl Model {
    #[cfg(target_arch = "wasm32")]
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
        let token = match self.tokenizer.tokenizer().decode(&[next_token], false) {
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

#[derive(Clone)]
pub struct CandleChatCompleter {

}

impl CandleChatCompleter {
    pub fn new() -> Self {
        Self { }
    }
}

#[async_trait]
impl ChatCompleter for CandleChatCompleter {
    #[cfg(not(target_arch = "wasm32"))]
    async fn get_response(&mut self, messages: Vec<super::ChatCompletionMessage>, task_configs: Vec<TaskConfig>) -> Result<Pin<Box<dyn Stream<Item = Result<super::ChatCompletionResponse>> + Send>>> {

        let mut prompt = "".to_string();

        for message in messages {
            let role = match message.role {
                MessageRole::user => {
                    "Human"
                },
                MessageRole::system => {
                    "System"
                },
                MessageRole::assistant => {
                    "AI"
                },
                MessageRole::function => todo!(),
            };
            
            prompt += &format!("{}: {:?}\n", role, message.content);
        }

        prompt += "AI:";

        let mut model = Model::new(Args { cpu: false, tracing: false, verbose_prompt: false, mmlu_dir: None, temperature: None, top_p: None, seed: 299792458, model_id: None, model: WhichModel::V2, revision: None, weight_file: None, tokenizer: None, quantized: true, repeat_penalty: 1.1, repeat_last_n: 64, dtype: None })?;

        let stream = model.run(prompt, 5000);
        let stream = stream.map(|x| {
            match x {
                Ok(x) => {
                    println!("GOT RESPONSE: '{}'", x);
                    Ok(super::ChatCompletionResponse {
                        completion: x,
                        ..Default::default()
                    })
                },
                Err(e) => {
                    println!("ERROR GETTING CHAT RESPONSE: {}", e);
                    Err(e)
                },
            }
        });
        Ok(Box::pin(stream))
    }

    #[cfg(target_arch = "wasm32")]
    async fn get_response(&mut self, messages: Vec<super::ChatCompletionMessage>, task_configs: Vec<TaskConfig>) -> Result<Pin<Box<dyn Stream<Item = Result<super::ChatCompletionResponse>> + Send>>> {
        // Refer to existing wasm32 supported code above
        todo!()
    }
}
use reqwest::header::{HeaderMap, HeaderValue};
use tokio::sync::Semaphore;
use std::{error::Error, collections::HashMap, env};
use aura_api_helper::*;
use super::*;
use lazy_static::lazy_static;
use openai_api_rs::v1::{*, api::Client, image::ImageGenerationRequest, audio::AudioSpeechRequest};
use futures_util::{Stream, FutureExt, StreamExt, stream, TryStreamExt};
use async_trait::async_trait;

lazy_static! {
    pub static ref VOICE_NAME_BY_NAME: HashMap<String, String> = {
        let mut map = HashMap::new();
        map.insert("default".to_string(), "alloy".to_string());
        map.insert("narrator".to_string(), "echo".to_string());
        map.insert("kind-man-a".to_string(), "fable".to_string());
        map.insert("kind-man-b".to_string(), "onyx".to_string());
        map.insert("man-a".to_string(), "nova".to_string());
        map.insert("young-woman-a".to_string(), "shimmer".to_string());
        map
    };
}

#[derive(Debug, Clone)]
pub struct OpenAITTS {
    pub api_key: String
}

impl OpenAITTS {
    pub fn new_from_env() -> OpenAITTS {
        OpenAITTS { api_key: env::var("OPENAI_API_KEY").unwrap() }
    }
}

#[async_trait]
impl Synthesizer for OpenAITTS {
    async fn create_speech(&self, emotion: String, voice_name: String, text: String) -> Result<SynthesisResult> {
        let voice_name = VOICE_NAME_BY_NAME.get(&voice_name).unwrap();
        let client = Client::new(env::var("OPENAI_API_KEY").unwrap().to_string());
        let bytes = client.create_speech(AudioSpeechRequest::new("tts-1-hd".to_string(), voice_name.to_owned(), text))?;
        Ok(SynthesisResult { bytes: bytes.to_vec() })
    }
}

use reqwest::header::{HeaderMap, HeaderValue};
use tokio::sync::Semaphore;
use std::{error::Error, collections::HashMap, env};
use super::*;
use lazy_static::lazy_static;
use openai_api_rs::v1::{api::{OpenAIClient, OpenAIClientBuilder}, audio::AudioSpeechRequest, image::ImageGenerationRequest, *};
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
pub struct OpenAISynthesizer {
    pub api_key: String
}

impl OpenAISynthesizer {
    pub fn new_from_env() -> OpenAISynthesizer {
        OpenAISynthesizer { api_key: env::var("OPENAI_API_KEY").unwrap() }
    }
}

#[async_trait]
impl Synthesizer for OpenAISynthesizer {
    async fn create_speech(&self, emotion: String, voice_name: String, text: String) -> Result<SynthesisResult> {
        // TODO: Rework as this crate's implementation only allows outputting to a file
        // My own personal fork handles this better--will need to update fork to latest version but include respone with bytes
        todo!()
        /*
        let voice_name = VOICE_NAME_BY_NAME.get(&voice_name).unwrap();
        let client = OpenAIClientBuilder::new().with_api_key(env::var("OPENAI_API_KEY").unwrap().to_string()).build().unwrap();
        let response = client.audio_speech(AudioSpeechRequest::new("tts-1-hd".to_string(), voice_name.to_owned(), text, "".to_string())).await?;
        Ok(SynthesisResult { bytes: response.inner.bytes.to_vec(), ..Default::default() })
        */
    }
}

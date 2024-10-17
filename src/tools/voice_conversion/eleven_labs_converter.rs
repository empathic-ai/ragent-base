use bytes::Bytes;
use reqwest::header::HeaderMap;

use serde::{Deserialize, Serialize};
use std::{error::Error, env};
use anyhow::Result;
use tokio::sync::Semaphore;
use super::*;
use super::super::eleven_labs_helpers::VOICE_ID_BY_NAME;
use reqwest::multipart;
use async_trait::async_trait;

#[derive(Debug)]
pub struct ElevenLabsConverter {
    pub api_key: String,
    pub format: String,
    semaphore: Semaphore
}

#[derive(Serialize, Debug)]
pub struct VoiceSettings {
    pub stability: f32,
    pub similarity_boost: f32,
    pub style: f32,
    pub use_speaker_boost: bool
}

/*
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoiceConvertRequest {
    pub audio: String,
    pub model_id: String,
    pub voice_settings: Option<String>,
    pub seed: Option<i32>
}
*/

impl ElevenLabsConverter {
    pub fn new_from_env() -> Self {
        // pcm: pcm_24000
        Self { api_key: env::var("ELEVEN_LABS_KEY").unwrap(), format: "pcm_16000".to_string(), semaphore: Semaphore::new(5) }
    }
}

#[async_trait]
impl VoiceConverter for ElevenLabsConverter {

    async fn convert_voice(&self, voice_name: String, bytes: Vec<u8>) -> Result<VoiceConversionResult> {
        let _permit = self.semaphore.acquire().await?;

        let client = reqwest::ClientBuilder::new().build()?;
        let voice_id = VOICE_ID_BY_NAME.get(&voice_name).unwrap();
        let api_key = self.api_key.clone();

        let mut headers = HeaderMap::new();
        //headers.insert("accept", "*/*".parse().unwrap());
        //headers.insert("content-type", "multipart/form-data".parse().unwrap());
        headers.insert(
            "xi-api-key",
            api_key.parse().unwrap(),
        );

        let audio = base64::encode(bytes.clone());

        let form = multipart::Form::new()
            .part("audio", multipart::Part::bytes(bytes).file_name("test.wav").mime_str("audio/wav").unwrap())
            .part("model_id", multipart::Part::text("eleven_multilingual_sts_v2"));
            //.part("voice_settings", multipart::Part::text(VoiceSettings {

            //}));

        let response = client
            .post(
                format!("https://api.elevenlabs.io/v1/speech-to-speech/{}/stream?output_format={}", voice_id, self.format)
            ).multipart(form)
            //.json(&VoiceConvertRequest { audio: audio, model_id: "eleven_english_sts_v2".to_string(), voice_settings: None, seed: None })
            //.json(&format!("{{\"voicemodel_uuid\": \"{voice_uuid}\", \"pace\": {pace}, \"speech\": \"{speech}\"}}"))
            //.header("Authorization", format!("Bearer {}", self.access_token))
            .headers(headers)
            .send()
            .await?;
        
        let bytes = response.bytes().await?;

        Ok(VoiceConversionResult { bytes: bytes.to_vec() })
    }
}

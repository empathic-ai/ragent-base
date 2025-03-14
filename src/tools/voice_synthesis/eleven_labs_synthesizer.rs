use bevy::log::info;
use reqwest::header::HeaderMap;

use serde::{Deserialize, Serialize};
use std::{error::Error, env};
use anyhow::{Result, anyhow};
use tokio::sync::Semaphore;
use super::super::eleven_labs_helpers::VOICE_ID_BY_NAME;

use super::*;
use lazy_static::lazy_static;
use async_trait::async_trait;

#[derive(Debug)]
pub struct ElevenLabsSynthesizer {
    pub api_key: String,
    pub format: String,
    semaphore: Semaphore
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoiceSettings {
    pub stability: f32,
    // Range from 0.0-1.0
    pub similarity_boost: f32,
    // Range from 0.0-1.0
    pub style: f32
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoiceStreamRequest {
    pub text: String,
    pub model_id: String,
    pub voice_settings: VoiceSettings
    // For information on ElevenLabs concurrent requests limit, see: https://help.elevenlabs.io/hc/en-us/articles/14312733311761-How-many-requests-can-I-make-and-can-I-increase-it-
    // Currently defaults to 5 (Creator tier)
    //pub optimize_streaming_latency: u32
}

impl ElevenLabsSynthesizer {

    // mp3_44100_128
    // for esp32 -- format = pcm_16000, pcm_24000
    pub fn new_from_env() -> Self {
        Self { api_key: env::var("ELEVEN_LABS_KEY").unwrap(), format: "pcm_16000".to_string(), semaphore: Semaphore::new(5) }
    }
}

#[async_trait]
impl Synthesizer for ElevenLabsSynthesizer {
    /// Refresh the access token. It is recommended to run this command after creating the client
    //async fn text_to_speech(voice_id: String, text: String) -> Result<Vec<u8>, Box<dyn Error>> {
    async fn create_speech(&self, emotion: String, voice_name: String, text: String) -> Result<SynthesisResult> {
        let _permit = self.semaphore.acquire().await?;

        let client = reqwest::ClientBuilder::new().build()?;
        let voice_id = VOICE_ID_BY_NAME.get(&voice_name).unwrap();
        let api_key = self.api_key.clone();

        let mut headers = HeaderMap::new();
        headers.insert("accept", "application/json".parse().unwrap());
        headers.insert("content-type", "application/json".parse().unwrap());
        headers.insert(
            "xi-api-key",
            api_key.parse().unwrap(),
        );

        //let voice_id = "kZ3lgd4yZQYd3EtWZCpw".to_string(); //Adam - see more at https://api.elevenlabs.io/v1/voices
        //let _pace = 1;

        //let json = serde_json::to_string(&UberduckRequest{ voicemodel_uuid: voice_uuid, pace: 1, speech: text.clone() }).unwrap();
        //console::info!(text.clone());

        //unsafe {
         //   while last_id < voice_order_id - 2 {
        //        browser_sleep(100).await;
        //    }
        //}

        // Model options: eleven_multilingual_v2, eleven_turbo_v2, eleven_flash_v2_5
        // See: https://elevenlabs.io/docs/models
        let response = client
            .post(
                format!("https://api.elevenlabs.io/v1/text-to-speech/{}/stream?output_format={}", voice_id, self.format)
            )
            .json(&VoiceStreamRequest {
                text:text.clone(),
                model_id: "eleven_flash_v2_5".to_string(),
                voice_settings: VoiceSettings {
                    stability: 0.5,
                    similarity_boost: 1.0,
                    style: 1.0
                }
            })
            //.json(&format!("{{\"voicemodel_uuid\": \"{voice_uuid}\", \"pace\": {pace}, \"speech\": \"{speech}\"}}"))
            //.header("Authorization", format!("Bearer {}", self.access_token))
            .headers(headers)
            .send()
            .await?;
        //.json::<SessionRefresh>()
        //.await;

        //let context = web_sys::AudioContext::new().unwrap();

        if response.status() == reqwest::StatusCode::OK {
                let bytes = response.bytes().await?;
                /* 
                let x: &[u8] = &bytes;
        
                let z = js_sys::Uint8Array::from(x).buffer();
        
                let p = context.decode_audio_data(&z).unwrap();
        
                let src = context.create_buffer_source().unwrap();
                let x = wasm_bindgen_futures::JsFuture::from(p).await.unwrap();
        
                let audio_buffer: AudioBuffer = x.into();
                src.set_buffer(Some(&audio_buffer));
        
                src.connect_with_audio_node(&context.destination());
        
                unsafe {
                    while is_playing || last_id != voice_order_id - 1 {
                        browser_sleep(100).await;
                    }
                    is_playing = true;
                    last_id = voice_order_id;
                    src.start();
                    let f = Closure::wrap(Box::new(move || {
                        is_playing = false;
                    }) as Box<dyn FnMut()>);
        
                    src.set_onended(Some(f.as_ref().unchecked_ref()));
        
                    f.forget();
                }
                */
                Ok(SynthesisResult { bytes: bytes.to_vec(), ..Default::default() })
                //Ok(())
        } else {
            info!("Error: {}", response.status().to_string());
            info!("Text: {}", response.text().await.unwrap());
            
            //info!("Response: {}", response.text().await.unwrap());
            //dbg!(err);
            Err(anyhow!("Error synthesizing speech!"))
        }
    }
}

fn calculate_cost(model_name: String, input_tokens: u64, output_tokens: u64, is_cached: bool, use_batch_api: bool) -> Decimal {
    todo!()
}
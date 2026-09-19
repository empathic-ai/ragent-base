use bevy::log::info;
use reqwest::header::HeaderMap;

use serde::{Deserialize, Serialize};
use std::{env, str::FromStr};
use anyhow::{Result, anyhow};
use tokio::sync::Semaphore;
use rust_decimal::Decimal;

use super::super::eleven_labs_helpers::VOICE_ID_BY_NAME;
use super::*;

use async_trait::async_trait;

#[derive(Debug)]
pub struct ElevenLabsSynthesizer {
    pub api_key: String,
    pub format: String,
    semaphore: Semaphore,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoiceSettings {
    pub stability: f32,
    pub similarity_boost: f32,
    pub style: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VoiceStreamRequest {
    pub text: String,
    pub model_id: String,
    pub voice_settings: VoiceSettings,
}

impl ElevenLabsSynthesizer {
    // mp3_44100_128
    // for esp32 -- format = pcm_16000, pcm_24000
    pub fn new_from_env() -> Self {
        Self {
            api_key: env::var("ELEVEN_LABS_KEY").unwrap(),
            format: "pcm_16000".to_string(),
            semaphore: Semaphore::new(5),
        }
    }
}

#[async_trait]
impl Synthesizer for ElevenLabsSynthesizer {
    async fn create_speech(
        &self,
        emotion: String,
        voice_name: String,
        text: String,
    ) -> Result<SynthesisResult> {
        let _permit = self.semaphore.acquire().await?;

        let client = reqwest::ClientBuilder::new().build()?;

        let voice_id = VOICE_ID_BY_NAME
            .get(&voice_name)
            .ok_or_else(|| anyhow!("Unknown ElevenLabs voice alias: {}", voice_name))?;

        let mut headers = HeaderMap::new();
        headers.insert("accept", "application/json".parse().unwrap());
        headers.insert("content-type", "application/json".parse().unwrap());
        headers.insert(
            "xi-api-key",
            self.api_key.parse().unwrap(),
        );

        let model = "eleven_flash_v2_5";

        let response = client
            .post(format!(
                "https://api.elevenlabs.io/v1/text-to-speech/{}/stream?output_format={}",
                voice_id,
                self.format
            ))
            .json(&VoiceStreamRequest {
                text: text.clone(),
                model_id: model.to_string(),
                voice_settings: VoiceSettings {
                    stability: 0.5,
                    similarity_boost: 1.0,
                    style: 1.0,
                },
            })
            .headers(headers)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::OK {
            // ElevenLabs returns the billed character count in this header.
            let billed_characters = response
                .headers()
                .get("character-cost")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| Decimal::from_str(value).ok())
                // Fallback if ElevenLabs ever omits the header.
                .unwrap_or_else(|| Decimal::from(text.chars().count() as u64));

            let cost = calculate_cost(
                model,
                billed_characters,
            );

            let bytes = response.bytes().await?;

            Ok(SynthesisResult {
                bytes: bytes.to_vec(),
                cost,
            })
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            Err(anyhow!(
                "Eleven Labs failed to synthesize speech: {} {}",
                status,
                body
            ))
        }
    }
}

/// Estimate the USD API cost of an ElevenLabs TTS generation.
///
/// `billed_characters` should preferably come from ElevenLabs' `character-cost`
/// response header rather than being calculated locally.
fn calculate_cost(
    model_name: &str,
    billed_characters: Decimal,
) -> Decimal {
    let cost_per_1k_characters = match model_name {
        // Flash / Turbo API pricing: $0.05 / 1K characters
        "eleven_flash_v2_5"
        | "eleven_flash_v2"
        | "eleven_turbo_v2_5"
        | "eleven_turbo_v2" => Decimal::new(5, 2),

        // Multilingual v2 API pricing: $0.10 / 1K characters
        "eleven_multilingual_v2" => Decimal::new(10, 2),

        _ => {
            // Unknown pricing: don't silently invent a cost.
            return Decimal::ZERO;
        }
    };

    billed_characters * cost_per_1k_characters / Decimal::from(1000u64)
}
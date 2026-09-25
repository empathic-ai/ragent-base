use anyhow::{Result, anyhow};
use async_trait::async_trait;
use rust_decimal::Decimal;
use sherpa_onnx::{GenerationConfig, OfflineTts, OfflineTtsConfig};
use std::sync::Arc;

use super::{SynthesisResult, Synthesizer};

#[derive(Clone, Debug)]
pub enum SherpaVoiceConfig {
    Preset {
        speaker_id: i32,
    },
    Cloned {
        reference_audio: Vec<f32>,
        reference_sample_rate: u32,
        reference_text: Option<String>,
    },
}

pub struct SherpaSynthesizer {
    tts: Arc<OfflineTts>,
}

impl SherpaSynthesizer {
    pub fn new(config: OfflineTtsConfig) -> Result<Self> {
        let tts = OfflineTts::create(&config)
            .ok_or_else(|| anyhow!("sherpa-onnx could not create the offline TTS engine"))?;
        Ok(Self { tts: Arc::new(tts) })
    }

    pub fn synthesize(&self, text: &str, voice: SherpaVoiceConfig) -> Result<SynthesisResult> {
        let mut generation = GenerationConfig::default();
        match voice {
            SherpaVoiceConfig::Preset { speaker_id } => generation.sid = speaker_id,
            SherpaVoiceConfig::Cloned {
                reference_audio,
                reference_sample_rate,
                reference_text,
            } => {
                generation.reference_audio = Some(reference_audio);
                generation.reference_sample_rate = reference_sample_rate as i32;
                generation.reference_text = reference_text;
            }
        }
        let audio = self
            .tts
            .generate_with_config::<fn(&[f32], f32) -> bool>(text, &generation, None)
            .ok_or_else(|| anyhow!("sherpa-onnx failed to synthesize audio"))?;
        Ok(SynthesisResult {
            bytes: pcm_f32_to_wav(audio.samples(), audio.sample_rate() as u32),
            cost: Decimal::ZERO,
            usage_unit: "characters".into(),
            usage_quantity: text.chars().count() as f32,
        })
    }
}

#[async_trait]
impl Synthesizer for SherpaSynthesizer {
    async fn create_speech(
        &self,
        _emotion: String,
        voice_name: String,
        text: String,
    ) -> Result<SynthesisResult> {
        let speaker_id = voice_name
            .strip_prefix("speaker:")
            .unwrap_or(&voice_name)
            .parse::<i32>()
            .unwrap_or(0);
        self.synthesize(&text, SherpaVoiceConfig::Preset { speaker_id })
    }
}

fn pcm_f32_to_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut wav = Vec::with_capacity(data_len as usize + 44);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    wav
}

#[cfg(test)]
mod tests {
    use super::pcm_f32_to_wav;

    #[test]
    fn writes_mono_pcm16_wav() {
        let wav = pcm_f32_to_wav(&[-1.0, 0.0, 1.0], 16_000);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 6);
        assert_eq!(wav.len(), 50);
    }
}

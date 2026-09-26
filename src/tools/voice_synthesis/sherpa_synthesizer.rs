use anyhow::{Result, anyhow};
use async_trait::async_trait;
use rust_decimal::Decimal;
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig,
    OfflineTtsPocketModelConfig,
};
use std::sync::Arc;

use super::{SynthesisResult, SynthesisStream, Synthesizer};

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

#[derive(Clone)]
pub struct SherpaSynthesizer {
    tts: Arc<OfflineTts>,
    default_voice: Option<SherpaVoiceConfig>,
}

impl SherpaSynthesizer {
    pub fn new(config: OfflineTtsConfig) -> Result<Self> {
        let tts = OfflineTts::create(&config)
            .ok_or_else(|| anyhow!("sherpa-onnx could not create the offline TTS engine"))?;
        Ok(Self {
            tts: Arc::new(tts),
            default_voice: None,
        })
    }

    pub fn from_pocket_tts(
        lm_flow: impl Into<String>,
        lm_main: impl Into<String>,
        encoder: impl Into<String>,
        decoder: impl Into<String>,
        text_conditioner: impl Into<String>,
        vocab_json: impl Into<String>,
        token_scores_json: impl Into<String>,
    ) -> Result<Self> {
        Self::new(OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                num_threads: 2,
                pocket: OfflineTtsPocketModelConfig {
                    voice_embedding_cache_capacity: 4,
                    lm_flow: Some(lm_flow.into()),
                    lm_main: Some(lm_main.into()),
                    encoder: Some(encoder.into()),
                    decoder: Some(decoder.into()),
                    text_conditioner: Some(text_conditioner.into()),
                    vocab_json: Some(vocab_json.into()),
                    token_scores_json: Some(token_scores_json.into()),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        })
    }

    pub fn with_default_voice(mut self, voice: SherpaVoiceConfig) -> Self {
        self.default_voice = Some(voice);
        self
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
        let started = std::time::Instant::now();
        let audio = self
            .tts
            .generate_with_config::<fn(&[f32], f32) -> bool>(text, &generation, None)
            .ok_or_else(|| anyhow!("sherpa-onnx failed to synthesize audio"))?;
        let gain = speech_gain(audio.samples());
        tracing::info!(elapsed_ms = started.elapsed().as_millis(), samples = audio.samples().len(), sample_rate = audio.sample_rate(), gain, "Local TTS synthesis metrics");
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
    async fn create_speech_stream(
        &self,
        _emotion: String,
        voice_name: String,
        text: String,
    ) -> Result<Option<SynthesisStream>> {
        let voice = self.default_voice.clone().unwrap_or_else(|| {
            SherpaVoiceConfig::Preset {
                speaker_id: voice_name.strip_prefix("speaker:").unwrap_or(&voice_name).parse().unwrap_or(0),
            }
        });
        let mut generation = GenerationConfig::default();
        match voice {
            SherpaVoiceConfig::Preset { speaker_id } => generation.sid = speaker_id,
            SherpaVoiceConfig::Cloned { reference_audio, reference_sample_rate, reference_text } => {
                generation.reference_audio = Some(reference_audio);
                generation.reference_sample_rate = reference_sample_rate as i32;
                generation.reference_text = reference_text;
            }
        }
        let format = delune::AudioFormat::new(self.tts.sample_rate() as u32, 1, 16);
        let (sender, chunks) = tokio::sync::mpsc::channel(8);
        let (finished, completion) = tokio::sync::oneshot::channel();
        let tts = self.tts.clone();
        tokio::task::spawn_blocking(move || {
            let started = std::time::Instant::now();
            let mut first = true;
            let audio = tts.generate_with_config(&text, &generation, Some(move |samples: &[f32], _progress| {
                if first && !samples.is_empty() {
                    tracing::info!(elapsed_ms = started.elapsed().as_millis(), "Local TTS first PCM chunk");
                    first = false;
                }
                for chunk in samples.chunks(480) {
                    let pcm = chunk.iter().map(|sample| {
                        let amplified = if sample.is_finite() { sample * 5.0 } else { 0.0 };
                        (amplified.clamp(-0.95, 0.95) * 32767.0) as i16
                    }).collect();
                    if sender.blocking_send(pcm).is_err() {
                        return false;
                    }
                }
                true
            }));
            let result = audio.ok_or_else(|| anyhow!("sherpa-onnx failed to synthesize audio"))
                .map(|_| SynthesisResult {
                    bytes: Vec::new(),
                    cost: Decimal::ZERO,
                    usage_quantity: text.chars().count() as f32,
                    usage_unit: "characters".into(),
                });
            let _ = finished.send(result);
        });
        Ok(Some(SynthesisStream { format, chunks, completion }))
    }

    async fn create_speech(
        &self,
        _emotion: String,
        voice_name: String,
        text: String,
    ) -> Result<SynthesisResult> {
        let voice = self.default_voice.clone().unwrap_or_else(|| {
            let speaker_id = voice_name
                .strip_prefix("speaker:")
                .unwrap_or(&voice_name)
                .parse::<i32>()
                .unwrap_or(0);
            SherpaVoiceConfig::Preset { speaker_id }
        });
        let synthesizer = self.clone();
        tokio::task::spawn_blocking(move || synthesizer.synthesize(&text, voice)).await?
    }
}

fn pcm_f32_to_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let gain = speech_gain(samples);
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
        let sample = if sample.is_finite() { sample * gain } else { 0.0 };
        wav.extend_from_slice(&((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    wav
}

fn speech_gain(samples: &[f32]) -> f32 {
    let mut peak = 0.0f32;
    let mut energy = 0.0f64;
    let mut active = 0usize;
    for &sample in samples {
        if !sample.is_finite() {
            continue;
        }
        peak = peak.max(sample.abs());
        if sample.abs() >= 0.001 {
            energy += f64::from(sample).powi(2);
            active += 1;
        }
    }
    if active == 0 {
        return 1.0;
    }
    let rms = (energy / active as f64).sqrt() as f32;
    (0.1 / rms).clamp(1.0, 4.0).min(0.95 / peak)
}

#[cfg(test)]
mod tests {
    use super::{pcm_f32_to_wav, speech_gain};

    #[test]
    fn gain_is_bounded_and_peak_safe() {
        assert_eq!(speech_gain(&[0.0; 8]), 1.0);
        assert_eq!(speech_gain(&[0.01, -0.01]), 4.0);
        assert!(speech_gain(&[0.9, 0.01]) * 0.9 <= 0.95);
        assert!(speech_gain(&[f32::NAN, 0.02]).is_finite());
    }

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

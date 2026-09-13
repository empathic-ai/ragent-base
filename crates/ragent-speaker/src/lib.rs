//! Speaker labels are evidence, never durable identities. Audio offsets are
//! integer sample positions on one source timeline; all queues/history are bounded.
mod fusion;
#[cfg(feature = "polyvoice")]
mod polyvoice;
#[cfg(feature = "pyannote")]
mod pyannote;
mod store;
mod timeline;
mod worker;

pub use fusion::*;
#[cfg(feature = "polyvoice")]
pub use polyvoice::*;
#[cfg(feature = "pyannote")]
pub use pyannote::*;
pub use store::*;
pub use timeline::*;
use tokio_util::sync::CancellationToken;
pub use worker::*;

use anyhow::{Result, ensure};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{fmt, sync::Arc};
use tokio::sync::mpsc;

pub const SAMPLE_RATE: u32 = 16_000;
pub const MAX_CLIP_SAMPLES: usize = SAMPLE_RATE as usize * 30;

#[derive(Clone)]
pub struct SpeakerAudio {
    pub samples: Vec<f32>,
}
impl SpeakerAudio {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.samples.is_empty() && self.samples.len() <= MAX_CLIP_SAMPLES,
            "audio must contain 0–30 seconds of 16 kHz mono samples"
        );
        ensure!(
            self.samples.iter().all(|v| v.is_finite() && v.abs() <= 1.0),
            "invalid PCM samples"
        );
        Ok(())
    }
    pub fn duration_seconds(&self) -> f64 {
        self.samples.len() as f64 / SAMPLE_RATE as f64
    }
    pub fn wav(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let size = self.samples.len() as u32 * 2;
        let mut out = Vec::with_capacity(size as usize + 44);
        out.extend(b"RIFF");
        out.extend((size + 36).to_le_bytes());
        out.extend(b"WAVEfmt ");
        out.extend(16u32.to_le_bytes());
        out.extend(1u16.to_le_bytes());
        out.extend(1u16.to_le_bytes());
        out.extend(SAMPLE_RATE.to_le_bytes());
        out.extend((SAMPLE_RATE * 2).to_le_bytes());
        out.extend(2u16.to_le_bytes());
        out.extend(16u16.to_le_bytes());
        out.extend(b"data");
        out.extend(size.to_le_bytes());
        for sample in &self.samples {
            out.extend(((*sample * 32767.0).round() as i16).to_le_bytes());
        }
        Ok(out)
    }
}
impl fmt::Debug for SpeakerAudio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpeakerAudio")
            .field("samples", &self.samples.len())
            .finish()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SpeakerLabel {
    pub provider: String,
    pub session: String,
    pub local: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiarizationSegment {
    pub label: SpeakerLabel,
    pub span: SampleSpan,
    pub overlap: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum VoiceTemplate {
    Embedding(Vec<f32>),
    Opaque(String),
}
impl fmt::Debug for VoiceTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VoiceTemplate([redacted])")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VoiceProfile {
    pub user_id: String,
    pub model_id: String,
    pub template: VoiceTemplate,
    pub sample_count: u32,
}

#[derive(Clone, Debug)]
pub struct VoiceScore {
    pub user_id: String,
    pub score: f32,
}
#[derive(Clone)]
pub struct VoiceEvidence {
    pub model_id: String,
    pub scores: Vec<VoiceScore>,
    pub embedding: Option<Vec<f32>>,
}

#[async_trait]
pub trait SpeakerEmbedder: Send + Sync {
    fn model_id(&self) -> &str;
    async fn embed(&self, audio: &SpeakerAudio) -> Result<Vec<f32>>;
}

/// Providers propose scores. Fusion alone chooses the durable user id.
#[async_trait]
pub trait SpeakerRecognizer: Send + Sync {
    fn model_id(&self) -> &str;
    async fn recognize(
        &self,
        audio: &SpeakerAudio,
        profiles: &[VoiceProfile],
    ) -> Result<VoiceEvidence>;
    async fn enroll(&self, audio: &SpeakerAudio) -> Result<VoiceTemplate>;
}

pub struct EmbeddingRecognizer {
    pub embedder: Arc<dyn SpeakerEmbedder>,
}
#[async_trait]
impl SpeakerRecognizer for EmbeddingRecognizer {
    fn model_id(&self) -> &str {
        self.embedder.model_id()
    }
    async fn recognize(
        &self,
        audio: &SpeakerAudio,
        profiles: &[VoiceProfile],
    ) -> Result<VoiceEvidence> {
        audio.validate()?;
        let embedding = unit_vector(&self.embedder.embed(audio).await?)?;
        let scores = profiles
            .iter()
            .filter(|p| p.model_id == self.model_id())
            .filter_map(|profile| {
                if let VoiceTemplate::Embedding(v) = &profile.template {
                    cosine(v, &embedding).map(|score| VoiceScore {
                        user_id: profile.user_id.clone(),
                        score,
                    })
                } else {
                    None
                }
            })
            .collect();
        Ok(VoiceEvidence {
            model_id: self.model_id().into(),
            scores,
            embedding: Some(embedding),
        })
    }
    async fn enroll(&self, audio: &SpeakerAudio) -> Result<VoiceTemplate> {
        audio.validate()?;
        Ok(VoiceTemplate::Embedding(unit_vector(
            &self.embedder.embed(audio).await?,
        )?))
    }
}

#[async_trait]
pub trait StreamingDiarizer: Send + Sync {
    /// Each frame is PCM16 LE, with explicit source sample offsets. A gap
    /// closes the provider session; implementations may reconnect with new labels.
    async fn run(
        &self,
        audio: mpsc::Receiver<AudioFrame>,
        events: mpsc::Sender<Result<DiarizationSegment>>,
        cancel: CancellationToken,
    ) -> Result<()>;
}

mod pipeline;
pub use pipeline::*;

impl fmt::Debug for VoiceEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VoiceEvidence")
            .field("model_id", &self.model_id)
            .field("candidate_count", &self.scores.len())
            .field("embedding", &self.embedding.as_ref().map(|v| v.len()))
            .finish()
    }
}

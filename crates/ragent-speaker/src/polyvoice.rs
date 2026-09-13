use crate::{SpeakerAudio, SpeakerEmbedder};
use ::polyvoice::{Embedder, ResNet34Native};
use anyhow::{Result, ensure};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc};

/// Fingerprints the actual model bytes so two weight files cannot accidentally
/// share a gallery. No network downloads or expensive inference on async threads.
#[derive(Clone)]
pub struct PolyvoiceEmbedder {
    inner: Arc<ResNet34Native>,
    model_id: String,
    permit: Arc<tokio::sync::Semaphore>,
}
impl PolyvoiceEmbedder {
    pub fn from_onnx_path(path: impl AsRef<Path>) -> Result<Self> {
        let bytes = std::fs::read(path.as_ref())?;
        let model_id = format!("polyvoice-0.19/resnet34/{:x}", Sha256::digest(&bytes));
        Ok(Self {
            inner: Arc::new(ResNet34Native::from_onnx_path(path)?),
            model_id,
            permit: Arc::new(tokio::sync::Semaphore::new(1)),
        })
    }
}
#[async_trait]
impl SpeakerEmbedder for PolyvoiceEmbedder {
    fn model_id(&self) -> &str {
        &self.model_id
    }
    async fn embed(&self, audio: &SpeakerAudio) -> Result<Vec<f32>> {
        audio.validate()?;
        ensure!(
            audio.duration_seconds() >= 1.,
            "embedding needs at least one second of speech"
        );
        let permit = self.permit.clone().acquire_owned().await?;
        let model = self.inner.clone();
        let samples = audio.samples.clone();
        Ok(tokio::task::spawn_blocking(move || {
            let _permit = permit;
            Embedder::embed(&*model, &samples)
        })
        .await??)
    }
}

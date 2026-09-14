//! Offline speaker evaluation. See docs/speaker-identification.md for the manifest format.
use anyhow::{Context, Result, ensure};
use ragent::speaker_identity::*;
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

#[derive(Deserialize)]
struct Clip {
    speaker: String,
    pcm: PathBuf,
}
#[derive(Deserialize)]
struct Dataset {
    enrollment: Vec<Clip>,
    evaluation: Vec<Clip>,
}
fn audio(root: &Path, clip: &Clip) -> Result<SpeakerAudio> {
    let bytes = std::fs::read(root.join(&clip.pcm))
        .with_context(|| format!("reading {}", clip.pcm.display()))?;
    ensure!(bytes.len() % 2 == 0, "PCM16 input has an incomplete sample");
    let audio = SpeakerAudio {
        samples: bytes
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
            .collect(),
    };
    audio.validate()?;
    Ok(audio)
}
#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        args.len() == 3,
        "usage: speaker_evaluate MODEL.onnx DATASET.json (mono PCM16 LE, 16 kHz)"
    );
    let manifest = Path::new(&args[2]);
    let dataset: Dataset = serde_json::from_slice(&std::fs::read(manifest)?)?;
    ensure!(
        !dataset.enrollment.is_empty() && !dataset.evaluation.is_empty(),
        "dataset must include enrollment and evaluation clips"
    );
    let root = manifest.parent().unwrap_or(Path::new("."));
    let recognizer = EmbeddingRecognizer {
        embedder: Arc::new(PolyvoiceEmbedder::from_onnx_path(&args[1])?),
    };
    let mut profiles = Vec::new();
    for clip in &dataset.enrollment {
        ensure!(
            !profiles
                .iter()
                .any(|p: &VoiceProfile| p.user_id == clip.speaker),
            "one enrollment clip per speaker required"
        );
        let audio = audio(root, clip)?;
        ensure!(
            audio.duration_seconds() >= 8.,
            "enrollment needs at least eight seconds"
        );
        profiles.push(VoiceProfile {
            user_id: clip.speaker.clone(),
            model_id: recognizer.model_id().into(),
            template: recognizer.enroll(&audio).await?,
            sample_count: 1,
        });
    }
    for (index, clip) in dataset.evaluation.iter().enumerate() {
        ensure!(
            !dataset.enrollment.iter().any(|e| e.pcm == clip.pcm),
            "evaluation must use held-out recordings"
        );
        let evidence = recognizer.recognize(&audio(root, clip)?, &profiles).await?;
        // Independent trials intentionally do not inherit another clip's identity.
        let resolution = FusionState::new(FusionConfig::default())?.resolve(
            &index.to_string(),
            None,
            &evidence,
            Instant::now(),
        );
        let mut ranked = evidence.scores;
        ranked.sort_by(|a, b| b.score.total_cmp(&a.score));
        let scores: Vec<_> = ranked
            .iter()
            .map(|s| serde_json::json!({"speaker": s.user_id, "score": s.score}))
            .collect();
        println!(
            "{}",
            serde_json::json!({"clip": clip.pcm, "expected": clip.speaker, "enrolled": profiles.iter().any(|p| p.user_id == clip.speaker), "scores": scores, "resolution": resolution, "model": recognizer.model_id()})
        );
    }
    Ok(())
}

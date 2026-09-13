use super::*;
use crate::prelude::*;
use anyhow::Result;
use bevy::prelude::*;
use bytes::Bytes;
use flux::prelude::*;
use futures_lite::StreamExt;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

/// Host-selected providers and a gallery scoped to this space. Never reuse a
/// mutable session across spaces; the store decides the authorized candidate set.
pub struct SpeakerOptions {
    pub recognizer: Arc<dyn SpeakerRecognizer>,
    pub store: Arc<dyn VoiceProfileStore>,
    pub diarizer: Option<Arc<dyn StreamingDiarizer>>,
    pub fusion: FusionConfig,
}

/// Asynchronous annotation. Neither this event nor a provider's local label
/// changes the authenticated sender on an existing SpeakEvent.
#[derive(Reflect, Reactive, documented::Documented, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpeakerResolvedEvent {
    pub utterance_id: String,
    pub source_session: String,
    pub start_sample: u64,
    pub end_sample: u64,
    pub local_speaker: Option<String>,
    pub user_id: Option<String>,
    pub candidate_id: Option<String>,
    pub confidence: String,
    pub enrollment_candidate: Option<String>,
}

/// Weak corroboration for an utterance and an enrolled user ID supplied by the
/// host's candidate list. Never use this to create a profile or authenticate a user.
#[derive(Reflect, Reactive, ragent_derive::Task, documented::Documented, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpeakerHintEvent {
    pub utterance_id: String,
    pub candidate_id: String,
}

async fn stopped(token: &common::prelude::CancellationToken) {
    while !token.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
impl TranscriberWorker {
    /// Uses any Transcriber that supplies final timestamps, source offsets and
    /// session IDs. Untimed transcribers still emit text, with no guessed identity.
    pub async fn new_with_speakers(
        space_id: Id,
        mut transcriber: Box<dyn Transcriber>,
        options: SpeakerOptions,
        output_tx: tokio::sync::broadcast::Sender<UserEvent>,
    ) -> Result<(Self, SpeakerPipeline)> {
        let identity =
            IdentitySession::new(options.recognizer, options.store, options.fusion).await?;
        let (pipeline, mut identities) = SpeakerPipeline::start(identity, options.diarizer)?;
        let token = common::prelude::CancellationToken::new();
        let (input_tx, mut input_rx) = tokio::sync::broadcast::channel::<Bytes>(64);
        let (asr_tx, asr_rx) = tokio::sync::broadcast::channel::<Bytes>(64);
        let mut transcripts = transcriber
            .transcribe_stream(16000, asr_rx, token.clone())
            .await?;
        let valid = Arc::new(AtomicBool::new(true));
        let (capture, capture_token, capture_valid) =
            (pipeline.clone(), token.clone(), valid.clone());
        let ingest = tokio::spawn(async move {
            let mut offset = 0u64;
            loop {
                let bytes = tokio::select! {
                    _ = stopped(&capture_token) => break,
                    value = input_rx.recv() => match value {
                        Ok(v) => v,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            // Lost byte count is unknowable. Continue text, abstain
                            // from identities until the host replaces this worker.
                            capture_valid.store(false, Ordering::SeqCst); continue;
                        }
                    }
                };
                if bytes.len() % 2 != 0 {
                    capture_valid.store(false, Ordering::SeqCst);
                }
                // Bound the size of every queued downstream audio item. Record
                // before sending to ASR so fast results can always find their audio.
                for chunk in bytes.chunks(640) {
                    let pcm16 = Bytes::copy_from_slice(chunk);
                    if capture_valid.load(Ordering::SeqCst) {
                        if capture
                            .push_audio(AudioFrame {
                                start_sample: offset,
                                pcm16: pcm16.clone(),
                            })
                            .is_err()
                        {
                            // Live-1 restarts on queue loss; it never borrows the ASR label.
                            log::warn!("Speaker audio processing discontinuity");
                        }
                    }
                    offset += pcm16.len() as u64 / 2;
                    if asr_tx.send(pcm16).is_err() {
                        return;
                    }
                    tokio::task::yield_now().await;
                }
            }
        });
        let (resolve, transcript_token, text_output) =
            (pipeline.clone(), token.clone(), output_tx.clone());
        let text = tokio::spawn(async move {
            loop {
                let response = tokio::select! { _ = stopped(&transcript_token) => break, value = transcripts.next() => match value { Some(v) => v, None => break } };
                let response = match response {
                    Ok(v) => v,
                    Err(_) => {
                        log::warn!("Transcriber returned an error");
                        continue;
                    }
                };
                if response.transcript.trim().is_empty() {
                    continue;
                }
                let utterance = uuid::Uuid::new_v4().to_string();
                let local = if resolve.uses_external_diarizer() {
                    None
                } else {
                    response.diarization_label.clone()
                };
                let text = format!(
                    "[Utterance:{} Speaker:{}] {}",
                    utterance,
                    local.as_deref().unwrap_or("Unknown"),
                    response.transcript
                );
                let mut event = UserEvent::new(Id::nil(), space_id, SpeakEvent { text });
                event.user_id = None;
                if text_output.send(event).is_err() {
                    break;
                }
                if !response.is_final || !valid.load(Ordering::SeqCst) {
                    continue;
                }
                if let (Some(start), Some(end), Some(origin), Some(session)) = (
                    response.start_seconds,
                    response.end_seconds,
                    response.stream_start_sample,
                    response.session_id,
                ) {
                    if let Ok(relative) = SampleSpan::from_seconds(start, end) {
                        if let (Some(start), Some(end)) = (
                            origin.checked_add(relative.start),
                            origin.checked_add(relative.end),
                        ) {
                            let turn = TranscriptTurn {
                                utterance_id: utterance,
                                source_session: session.clone(),
                                span: SampleSpan { start, end },
                                label: local.map(|local| SpeakerLabel {
                                    provider: "transcriber".into(),
                                    session,
                                    local,
                                }),
                            };
                            if resolve.submit(turn).is_err() {
                                log::debug!(
                                    "Speaker turn unavailable or queue full; transcript remains anonymous"
                                );
                            }
                        }
                    }
                }
            }
        });
        let events_token = token.clone();
        let events = tokio::spawn(async move {
            loop {
                let value = tokio::select! { _ = stopped(&events_token) => break, v = identities.recv() => match v { Some(v) => v, None => break } };
                let value = match value {
                    Ok(v) => v,
                    Err(_) => {
                        log::warn!("Speaker recognition failed; transcript remains anonymous");
                        continue;
                    }
                };
                let mut event = UserEvent::new(
                    Id::nil(),
                    space_id,
                    SpeakerResolvedEvent {
                        utterance_id: value.utterance_id,
                        source_session: value.source_session,
                        start_sample: value.span.start,
                        end_sample: value.span.end,
                        local_speaker: value.label.map(|v| v.local),
                        user_id: value.resolution.user_id,
                        candidate_id: value.resolution.candidate_id,
                        confidence: match value.resolution.confidence {
                            IdentityConfidence::High => "high",
                            IdentityConfidence::Medium => "medium",
                            IdentityConfidence::Unknown => "unknown",
                        }
                        .into(),
                        enrollment_candidate: value.enrollment_candidate,
                    },
                );
                event.user_id = None;
                if output_tx.send(event).is_err() {
                    break;
                }
            }
        });
        Ok((
            Self::from_speaker_parts(token, input_tx, vec![ingest, text, events]),
            pipeline,
        ))
    }
}

impl SpeakerOptions {
    pub async fn from_env(store: Arc<dyn VoiceProfileStore>) -> Result<Self> {
        let recognizer: Arc<dyn SpeakerRecognizer> = match std::env::var("SPEAKER_RECOGNIZER")
            .as_deref()
            .unwrap_or("polyvoice")
        {
            #[cfg(feature = "polyvoice")]
            "polyvoice" => {
                let path = std::env::var("POLYVOICE_MODEL_PATH").map_err(|_| {
                    anyhow::anyhow!("Set POLYVOICE_MODEL_PATH to ResNet34 INT8 ONNX weights")
                })?;
                let embedder =
                    tokio::task::spawn_blocking(move || PolyvoiceEmbedder::from_onnx_path(path))
                        .await??;
                Arc::new(EmbeddingRecognizer {
                    embedder: Arc::new(embedder),
                })
            }
            #[cfg(feature = "pyannote")]
            "pyannote" => Arc::new(PyannoteVoiceprints::new(PyannoteClient::from_env()?)),
            _ => anyhow::bail!("Unsupported SPEAKER_RECOGNIZER or missing provider feature"),
        };
        let diarizer: Option<Arc<dyn StreamingDiarizer>> = match std::env::var("SPEAKER_DIARIZER")
            .as_deref()
            .unwrap_or("deepgram")
        {
            "deepgram" => None,
            #[cfg(feature = "pyannote")]
            "live-1" => Some(Arc::new(Live1Diarizer::new(PyannoteClient::from_env()?))),
            _ => anyhow::bail!("Unsupported SPEAKER_DIARIZER or missing pyannote feature"),
        };
        let fusion = match std::env::var("SPEAKER_FUSION_CONFIG") {
            Ok(value) => serde_json::from_str::<FusionConfig>(&value)?,
            Err(std::env::VarError::NotPresent) => FusionConfig::default(),
            Err(error) => return Err(error.into()),
        };
        fusion.validate()?;
        Ok(Self {
            recognizer,
            store,
            diarizer,
            fusion,
        })
    }
}
impl SpaceWorker {
    pub async fn new_with_speakers(
        space_id: Id,
        transcriber: Box<dyn Transcriber>,
        options: SpeakerOptions,
    ) -> Result<Self> {
        let (output_tx, output_rx) = tokio::sync::broadcast::channel(64);
        let (space_transcriber, speaker_pipeline) =
            TranscriberWorker::new_with_speakers(space_id, transcriber, options, output_tx.clone())
                .await?;
        Ok(Self {
            state: Arc::new(futures_util::lock::Mutex::new(SpaceState {
                space_id,
                token: common::prelude::CancellationToken::new(),
                space_transcriber,
                user_transcribers: Default::default(),
                use_transcribers: true,
                output_tx,
                output_rx,
                mic_input: None,
                speaker_pipeline: Some(speaker_pipeline),
            })),
        })
    }
}

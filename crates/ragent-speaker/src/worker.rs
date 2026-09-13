use crate::*;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct IdentityTurn {
    pub utterance_id: String,
    pub source_session: String,
    pub span: SampleSpan,
    pub label: Option<SpeakerLabel>,
    pub audio: SpeakerAudio,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IdentityEvent {
    pub utterance_id: String,
    pub source_session: String,
    pub span: SampleSpan,
    pub label: Option<SpeakerLabel>,
    pub resolution: Resolution,
    pub enrollment_candidate: Option<String>,
}
struct UnknownCluster {
    id: String,
    model: String,
    centroid: Option<Vec<f32>>,
    audio: Vec<f32>,
    created: Instant,
    announced: bool,
}
/// One session owns fusion and bounded enrollment buffers. Calls are serialized
/// by the host's bounded worker; provider latency never blocks the transcriber.
pub struct IdentitySession {
    recognizer: Arc<dyn SpeakerRecognizer>,
    store: Arc<dyn VoiceProfileStore>,
    profiles: Vec<VoiceProfile>,
    fusion: FusionState,
    unknown: VecDeque<UnknownCluster>,
    turn_labels: VecDeque<(String, SpeakerLabel)>,
    pending_hint: Option<(SpeakerLabel, String, Instant)>,
    adapt_embeddings: bool,
    last_label_session: Option<(String, String)>,
    seen: VecDeque<String>,
    session: Option<String>,
    last_end: Option<u64>,
}
impl IdentitySession {
    pub async fn new(
        recognizer: Arc<dyn SpeakerRecognizer>,
        store: Arc<dyn VoiceProfileStore>,
        config: FusionConfig,
    ) -> Result<Self> {
        let profiles = store.load().await?;
        validate_profiles(&profiles)?;
        Ok(Self {
            recognizer,
            store,
            profiles,
            turn_labels: VecDeque::new(),
            pending_hint: None,
            adapt_embeddings: false,
            last_label_session: None,
            fusion: FusionState::new(config)?,
            unknown: VecDeque::new(),
            seen: VecDeque::new(),
            session: None,
            last_end: None,
        })
    }
    /// Enable only after model-specific calibration. Opaque voiceprints always require explicit re-enrollment.
    pub fn set_embedding_adaptation(&mut self, enabled: bool) {
        self.adapt_embeddings = enabled;
    }
    pub fn profiles(&self) -> &[VoiceProfile] {
        &self.profiles
    }
    pub fn reset(&mut self) {
        self.fusion.reset();
        self.last_label_session = None;
        self.turn_labels.clear();
        self.pending_hint = None;
        self.unknown.clear();
        self.seen.clear();
        self.last_end = None;
        self.session = None;
    }
    pub fn proximity(&mut self, user: &str, value: ProximityBucket) {
        if self.profiles.iter().any(|p| p.user_id == user) {
            self.fusion
                .set_proximity(user.into(), value, Instant::now());
        }
    }
    pub fn hint(&mut self, utterance: &str, user: &str) -> Result<()> {
        ensure!(
            self.profiles.iter().any(|p| p.user_id == user),
            "hint is outside enrolled candidate set"
        );
        let label = self
            .turn_labels
            .iter()
            .rev()
            .find(|(id, _)| id == utterance)
            .map(|(_, label)| label.clone())
            .ok_or_else(|| anyhow::anyhow!("hint utterance is unknown or expired"))?;
        self.pending_hint = Some((label, user.into(), Instant::now()));
        Ok(())
    }
    pub async fn resolve(&mut self, turn: IdentityTurn) -> Result<IdentityEvent> {
        turn.audio.validate()?;
        ensure!(
            turn.span.end > turn.span.start
                && turn.span.end - turn.span.start == turn.audio.samples.len() as u64,
            "audio/span mismatch"
        );
        if self
            .session
            .as_ref()
            .is_some_and(|s| s != &turn.source_session)
        {
            self.reset();
        }
        let label_session = turn
            .label
            .as_ref()
            .map(|l| (l.provider.clone(), l.session.clone()));
        if self.last_label_session.is_some()
            && label_session.is_some()
            && self.last_label_session != label_session
        {
            self.reset();
        }
        self.last_label_session = label_session;
        self.session = Some(turn.source_session.clone());
        let evidence_id = format!(
            "{}:{}:{}",
            turn.utterance_id, turn.span.start, turn.span.end
        );
        ensure!(!self.seen.contains(&evidence_id), "duplicate identity turn");
        ensure!(
            self.last_end.is_none_or(|end| turn.span.start >= end),
            "overlapping or out-of-order identity evidence"
        );
        self.last_end = Some(turn.span.end);
        if self.seen.len() == 1024 {
            self.seen.pop_front();
        }
        self.seen.push_back(evidence_id);
        let mut event = IdentityEvent {
            utterance_id: turn.utterance_id.clone(),
            source_session: turn.source_session.clone(),
            span: turn.span,
            label: turn.label.clone(),
            resolution: Resolution::unknown(),
            enrollment_candidate: None,
        };
        if turn.label.is_none() || !quality_ok(&turn.audio) {
            self.fusion.reset();
            return Ok(event);
        }
        if let Some(label) = &turn.label {
            if self.turn_labels.len() == 64 {
                self.turn_labels.pop_front();
            }
            self.turn_labels
                .push_back((turn.utterance_id.clone(), label.clone()));
        }
        let mut evidence = self
            .recognizer
            .recognize(&turn.audio, &self.profiles)
            .await?;
        ensure!(
            evidence.model_id == self.recognizer.model_id(),
            "recognizer model mismatch"
        );
        evidence.scores.retain(|s| {
            self.profiles
                .iter()
                .any(|p| p.user_id == s.user_id && p.model_id == evidence.model_id)
        });
        if let Some(v) = &mut evidence.embedding {
            *v = unit_vector(v)?;
        }
        if let Some((label, user, at)) = self.pending_hint.take() {
            if turn.label.as_ref() == Some(&label) && at.elapsed() < Duration::from_secs(45) {
                self.fusion.hint(turn.utterance_id.clone(), user, at);
            }
        }
        event.resolution = self.fusion.resolve(
            &turn.utterance_id,
            turn.label.as_ref(),
            &evidence,
            Instant::now(),
        );
        if event.resolution.confidence == IdentityConfidence::Unknown {
            event.enrollment_candidate = self.offer_unknown(&turn, &evidence);
        }
        if self.adapt_embeddings
            && event.resolution.confidence == IdentityConfidence::High
            && event.resolution.score >= 0.95
            && event.resolution.margin >= 0.2
        {
            if let (Some(user), Some(embedding)) = (&event.resolution.user_id, &evidence.embedding)
            {
                let mut next = self.profiles.clone();
                if let Some(profile) = next
                    .iter_mut()
                    .find(|p| &p.user_id == user && p.model_id == evidence.model_id)
                {
                    if let VoiceTemplate::Embedding(centroid) = &mut profile.template {
                        if cosine(centroid, embedding).unwrap_or(-1.) >= 0.9 {
                            let weight = (1.0 / (profile.sample_count as f32 + 1.0)).min(0.05);
                            *centroid = unit_vector(
                                &centroid
                                    .iter()
                                    .zip(embedding)
                                    .map(|(a, b)| a * (1. - weight) + b * weight)
                                    .collect::<Vec<_>>(),
                            )?;
                            profile.sample_count = profile.sample_count.saturating_add(1);
                            self.store.save(&next).await?;
                            self.profiles = next;
                        }
                    }
                }
            }
        }
        Ok(event)
    }
    fn offer_unknown(&mut self, turn: &IdentityTurn, evidence: &VoiceEvidence) -> Option<String> {
        self.unknown
            .retain(|c| c.created.elapsed() < Duration::from_secs(120));
        turn.label.as_ref()?;
        let index = self.unknown.iter().position(|c| {
            c.model == evidence.model_id
                && match (&c.centroid, &evidence.embedding) {
                    (Some(a), Some(b)) => cosine(a, b).unwrap_or(-1.) >= 0.85,
                    (None, None) => false,
                    _ => false,
                }
        });
        let index = match index {
            Some(i) => i,
            None => {
                if self.unknown.len() == 8 {
                    self.unknown.pop_front();
                }
                self.unknown.push_back(UnknownCluster {
                    id: uuid::Uuid::new_v4().to_string(),
                    model: evidence.model_id.clone(),
                    centroid: evidence.embedding.clone(),
                    audio: vec![],
                    created: Instant::now(),
                    announced: false,
                });
                self.unknown.len() - 1
            }
        };
        let cluster = &mut self.unknown[index];
        if let (Some(a), Some(b)) = (&mut cluster.centroid, &evidence.embedding) {
            if let Ok(next) = unit_vector(
                &a.iter()
                    .zip(b)
                    .map(|(a, b)| a * 0.9 + b * 0.1)
                    .collect::<Vec<_>>(),
            ) {
                *a = next;
            }
        }
        let remaining = (SAMPLE_RATE as usize * 10).saturating_sub(cluster.audio.len());
        // Only the first 10 seconds are retained, and no raw audio leaves this
        // private session in an enrollment notification.
        cluster
            .audio
            .extend(turn.audio.samples.iter().take(remaining));
        if cluster.audio.len() >= SAMPLE_RATE as usize * 8 && !cluster.announced {
            cluster.announced = true;
            Some(cluster.id.clone())
        } else {
            None
        }
    }
    /// Explicit host-confirmed enrollment. A model's name guess cannot call this
    /// through the ordinary event bus. Persist first; publish profile only on success.
    pub async fn confirm_enrollment(&mut self, candidate: &str, user_id: &str) -> Result<()> {
        ensure!(
            !user_id.is_empty() && user_id.len() <= 256,
            "invalid user id"
        );
        let cluster = self
            .unknown
            .iter()
            .find(|c| {
                c.id == candidate && c.announced && c.created.elapsed() < Duration::from_secs(120)
            })
            .ok_or_else(|| anyhow::anyhow!("enrollment candidate missing or expired"))?;
        let template = self
            .recognizer
            .enroll(&SpeakerAudio {
                samples: cluster.audio.clone(),
            })
            .await?;
        let mut next = self.profiles.clone();
        next.retain(|p| !(p.user_id == user_id && p.model_id == self.recognizer.model_id()));
        next.push(VoiceProfile {
            user_id: user_id.into(),
            model_id: self.recognizer.model_id().into(),
            template,
            sample_count: 1,
        });
        validate_profiles(&next)?;
        self.store.save(&next).await?;
        self.profiles = next;
        self.unknown.retain(|c| c.id != candidate);
        self.fusion.reset();
        Ok(())
    }
    /// Enroll a separately captured, host-confirmed single-speaker sample.
    pub async fn enroll_sample(&mut self, user_id: &str, audio: &SpeakerAudio) -> Result<()> {
        ensure!(
            !user_id.is_empty() && user_id.len() <= 256,
            "invalid user id"
        );
        audio.validate()?;
        ensure!(
            audio.duration_seconds() >= 8. && quality_ok(audio),
            "enrollment requires 8–30s of clean single-speaker audio"
        );
        let template = self.recognizer.enroll(audio).await?;
        let mut next = self.profiles.clone();
        next.retain(|p| !(p.user_id == user_id && p.model_id == self.recognizer.model_id()));
        next.push(VoiceProfile {
            user_id: user_id.into(),
            model_id: self.recognizer.model_id().into(),
            template,
            sample_count: 1,
        });
        validate_profiles(&next)?;
        self.store.save(&next).await?;
        self.profiles = next;
        self.fusion.reset();
        Ok(())
    }
    pub async fn forget(&mut self, user_id: &str) -> Result<()> {
        let next = self
            .profiles
            .iter()
            .filter(|p| p.user_id != user_id)
            .cloned()
            .collect::<Vec<_>>();
        self.store.save(&next).await?;
        self.profiles = next;
        self.reset();
        Ok(())
    }
}
fn quality_ok(audio: &SpeakerAudio) -> bool {
    if audio.samples.len() < SAMPLE_RATE as usize || audio.samples.len() > MAX_CLIP_SAMPLES {
        return false;
    }
    let rms = (audio
        .samples
        .iter()
        .map(|x| (*x as f64).powi(2))
        .sum::<f64>()
        / audio.samples.len() as f64)
        .sqrt();
    let clipped = audio.samples.iter().filter(|v| v.abs() >= 0.999).count() as f64
        / audio.samples.len() as f64;
    rms >= 0.005 && clipped < 0.02
}

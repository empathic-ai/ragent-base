use crate::{SpeakerLabel, VoiceEvidence};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdentityConfidence {
    High,
    Medium,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Resolution {
    /// Only High confidence commits an identity. Medium exposes a candidate separately.
    pub user_id: Option<String>,
    pub candidate_id: Option<String>,
    pub confidence: IdentityConfidence,
    pub score: f32,
    pub margin: f32,
}
impl Resolution {
    pub fn unknown() -> Self {
        Self {
            user_id: None,
            candidate_id: None,
            confidence: IdentityConfidence::Unknown,
            score: 0.,
            margin: 0.,
        }
    }
}

/// Scores are backend-specific evidence, not probabilities. Calibrate these
/// defaults on labeled recordings for the selected model/microphone.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct FusionConfig {
    pub high_voice_score: f32,
    pub medium_voice_score: f32,
    pub minimum_voice_margin: f32,
    pub consecutive_high_required: u32,
    pub half_life_seconds: f32,
}
impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            high_voice_score: 0.80,
            medium_voice_score: 0.65,
            minimum_voice_margin: 0.08,
            consecutive_high_required: 2,
            half_life_seconds: 20.,
        }
    }
}
impl FusionConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.medium_voice_score.is_finite()
                && self.high_voice_score.is_finite()
                && (0.0..=1.0).contains(&self.medium_voice_score)
                && (self.medium_voice_score..=1.0).contains(&self.high_voice_score),
            "invalid voice thresholds"
        );
        ensure!(
            self.minimum_voice_margin.is_finite()
                && (0.01..=1.).contains(&self.minimum_voice_margin),
            "invalid score margin"
        );
        ensure!(
            (1..=10).contains(&self.consecutive_high_required)
                && self.half_life_seconds.is_finite()
                && self.half_life_seconds > 0.,
            "invalid fusion hysteresis"
        );
        Ok(())
    }
}
#[derive(Clone, Copy, Debug)]
pub enum ProximityBucket {
    VeryClose,
    InRoom,
    NotDetected,
}
impl ProximityBucket {
    pub fn from_rssi(rssi: i32) -> Self {
        if rssi > -50 {
            Self::VeryClose
        } else if rssi > -75 {
            Self::InRoom
        } else {
            Self::NotDetected
        }
    }
    fn score(self) -> f32 {
        match self {
            Self::VeryClose => 0.02,
            Self::InRoom => 0.01,
            Self::NotDetected => 0.,
        }
    }
}
struct Belief {
    score: f32,
    at: Instant,
}
/// Bounded, session-scoped evidence. A new voice must pass the current acoustic
/// threshold and margin on every turn, regardless of accumulated hints/history.
pub struct FusionState {
    config: FusionConfig,
    model_id: Option<String>,
    beliefs: HashMap<String, Belief>,
    proximity: HashMap<String, (ProximityBucket, Instant)>,
    hint: Option<(String, String, Instant)>,
    previous: Option<(String, Option<Vec<f32>>, Option<SpeakerLabel>, Instant)>,
    consecutive: Option<(String, u32, Instant)>,
}
impl FusionState {
    pub fn new(config: FusionConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            model_id: None,
            beliefs: HashMap::new(),
            proximity: HashMap::new(),
            hint: None,
            previous: None,
            consecutive: None,
        })
    }
    pub fn set_proximity(&mut self, user: String, value: ProximityBucket, now: Instant) {
        self.proximity.insert(user, (value, now));
    }
    /// The host must scope candidate ids; one hint replaces the previous one.
    pub fn hint(&mut self, utterance: String, user: String, now: Instant) {
        self.hint = Some((utterance, user, now));
    }
    pub fn reset(&mut self) {
        self.beliefs.clear();
        self.previous = None;
        self.consecutive = None;
        self.hint = None;
        self.model_id = None;
    }
    pub fn resolve(
        &mut self,
        utterance: &str,
        label: Option<&SpeakerLabel>,
        evidence: &VoiceEvidence,
        now: Instant,
    ) -> Resolution {
        if self
            .model_id
            .as_ref()
            .is_some_and(|id| id != &evidence.model_id)
        {
            self.reset();
        }
        self.model_id = Some(evidence.model_id.clone());
        // Never let NaN, duplicate candidates, or incompatible embedding dimensions
        // enter scoring. Missing/invalid voice evidence always returns unknown.
        let mut scores = evidence.scores.clone();
        if scores
            .iter()
            .any(|s| !s.score.is_finite() || !(-1.0..=1.).contains(&s.score))
        {
            self.consecutive = None;
            return Resolution::unknown();
        }
        scores.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.user_id.cmp(&b.user_id)));
        let mut seen = std::collections::HashSet::new();
        if scores.iter().any(|s| !seen.insert(&s.user_id)) {
            self.consecutive = None;
            return Resolution::unknown();
        }
        let Some(best) = scores.first() else {
            self.consecutive = None;
            return Resolution::unknown();
        };
        // The unknown hypothesis competes even if only one enrolled user exists.
        let margin = best.score
            - scores
                .get(1)
                .map(|s| s.score)
                .unwrap_or(self.config.medium_voice_score - self.config.minimum_voice_margin);
        let acoustic_high = best.score >= self.config.high_voice_score
            && margin >= self.config.minimum_voice_margin;
        let acoustic_medium = best.score >= self.config.medium_voice_score
            && margin >= self.config.minimum_voice_margin;
        self.beliefs.retain(|id, _| seen.contains(id));
        self.proximity
            .retain(|_, (_, at)| now.saturating_duration_since(*at) < Duration::from_secs(30));
        let mut bonus = self
            .proximity
            .get(&best.user_id)
            .map(|(p, _)| p.score())
            .unwrap_or(0.);
        if let Some((id, embedding, old_label, at)) = &self.previous {
            if id == &best.user_id && now.saturating_duration_since(*at) < Duration::from_secs(20) {
                if label.is_some() && old_label.as_ref() == label {
                    bonus += 0.01;
                }
                if let (Some(a), Some(b)) = (embedding, &evidence.embedding) {
                    if cosine(a, b).unwrap_or(-1.) >= 0.8 {
                        bonus += 0.02;
                    }
                }
            }
        }
        if self.hint.as_ref().is_some_and(|(turn, id, at)| {
            turn == utterance
                && id == &best.user_id
                && now.saturating_duration_since(*at) < Duration::from_secs(45)
        }) {
            bonus += 0.02;
        }
        self.hint = None; // repeated tool calls cannot compound across turns
        let prior = self
            .beliefs
            .get(&best.user_id)
            .map(|b| {
                b.score
                    * 0.5_f32.powf(
                        now.saturating_duration_since(b.at).as_secs_f32()
                            / self.config.half_life_seconds,
                    )
            })
            .unwrap_or(0.);
        let fused = (best.score * 0.9 + prior * 0.1 + bonus).clamp(-1., 1.);
        self.beliefs.insert(
            best.user_id.clone(),
            Belief {
                score: fused,
                at: now,
            },
        );
        let count = if acoustic_high {
            match &self.consecutive {
                Some((id, n, at))
                    if id == &best.user_id
                        && now.saturating_duration_since(*at) < Duration::from_secs(20) =>
                {
                    n.saturating_add(1)
                }
                _ => 1,
            }
        } else {
            0
        };
        self.consecutive = (count > 0).then(|| (best.user_id.clone(), count, now));
        let confidence = if acoustic_high
            && fused >= self.config.high_voice_score
            && count >= self.config.consecutive_high_required
        {
            IdentityConfidence::High
        } else if acoustic_medium {
            IdentityConfidence::Medium
        } else {
            IdentityConfidence::Unknown
        };
        // Only committed acoustic identities can reinforce later continuity.
        self.previous = (confidence == IdentityConfidence::High).then(|| {
            (
                best.user_id.clone(),
                evidence.embedding.clone(),
                label.cloned(),
                now,
            )
        });
        Resolution {
            user_id: (confidence == IdentityConfidence::High).then(|| best.user_id.clone()),
            candidate_id: (confidence != IdentityConfidence::Unknown).then(|| best.user_id.clone()),
            confidence,
            score: best.score,
            margin,
        }
    }
}

pub fn unit_vector(v: &[f32]) -> Result<Vec<f32>> {
    ensure!(
        !v.is_empty() && v.len() <= 4096 && v.iter().all(|x| x.is_finite()),
        "invalid embedding"
    );
    let norm = v.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    ensure!(norm > 1e-12, "zero embedding");
    Ok(v.iter().map(|x| (*x as f64 / norm) as f32).collect())
}
pub fn cosine(a: &[f32], b: &[f32]) -> Option<f32> {
    if a.len() != b.len() {
        return None;
    }
    let a = unit_vector(a).ok()?;
    let b = unit_vector(b).ok()?;
    Some(
        a.iter()
            .zip(b)
            .map(|(x, y)| x * y)
            .sum::<f32>()
            .clamp(-1., 1.),
    )
}

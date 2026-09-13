use anyhow::Result;
use async_trait::async_trait;
use ragent_speaker::*;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
fn evidence(a: f32, b: f32) -> VoiceEvidence {
    VoiceEvidence {
        model_id: "test-v1".into(),
        scores: vec![
            VoiceScore {
                user_id: "alice".into(),
                score: a,
            },
            VoiceScore {
                user_id: "bob".into(),
                score: b,
            },
        ],
        embedding: Some(vec![a, b]),
    }
}
fn label(local: &str) -> SpeakerLabel {
    SpeakerLabel {
        provider: "test".into(),
        session: "session".into(),
        local: local.into(),
    }
}
#[test]
fn voice_gate_prevents_hints_or_previous_speaker_from_claiming_unknown() {
    let mut fusion = FusionState::new(FusionConfig::default()).unwrap();
    let now = Instant::now();
    assert_eq!(
        fusion
            .resolve("1", Some(&label("0")), &evidence(0.95, 0.1), now)
            .confidence,
        IdentityConfidence::Medium
    );
    assert_eq!(
        fusion
            .resolve("2", Some(&label("0")), &evidence(0.95, 0.1), now)
            .user_id
            .as_deref(),
        Some("alice")
    );
    for i in 0..20 {
        fusion.hint("3".into(), "alice".into(), now);
        fusion.set_proximity("alice".into(), ProximityBucket::VeryClose, now);
        assert_eq!(
            fusion
                .resolve("3", Some(&label("0")), &evidence(0.2, 0.1), now)
                .user_id,
            None
        );
        let _ = i;
    }
    assert_eq!(
        fusion
            .resolve("4", Some(&label("0")), &evidence(0.1, 0.95), now)
            .confidence,
        IdentityConfidence::Medium
    );
    assert_eq!(
        fusion
            .resolve("5", Some(&label("0")), &evidence(0.1, 0.95), now)
            .user_id
            .as_deref(),
        Some("bob")
    );
}
#[test]
fn ambiguity_invalid_scores_and_session_reset_abstain() {
    let mut f = FusionState::new(FusionConfig::default()).unwrap();
    let n = Instant::now();
    for _ in 0..4 {
        assert_eq!(
            f.resolve("t", None, &evidence(0.95, 0.94), n).confidence,
            IdentityConfidence::Unknown
        );
    }
    assert_eq!(
        f.resolve("t", None, &evidence(f32::NAN, 0.1), n).confidence,
        IdentityConfidence::Unknown
    );
    f.resolve("t", None, &evidence(0.95, 0.1), n);
    f.resolve("t", None, &evidence(0.95, 0.1), n);
    assert_eq!(
        f.resolve("t", None, &evidence(0.95, 0.1), n + Duration::from_secs(60))
            .confidence,
        IdentityConfidence::Medium
    );
    let mut e = evidence(0.95, 0.1);
    e.model_id = "new-model".into();
    assert_eq!(
        f.resolve("t", None, &e, n + Duration::from_secs(61))
            .confidence,
        IdentityConfidence::Medium
    );
}
#[test]
fn timeline_rejects_missing_evicted_and_overflowed_audio() {
    let mut t = AudioTimeline::new(1).unwrap();
    t.push(&AudioFrame {
        start_sample: 0,
        pcm16: vec![0; 32000].into(),
    })
    .unwrap();
    assert_eq!(
        t.clip(SampleSpan::new(0, 16000).unwrap())
            .unwrap()
            .samples
            .len(),
        16000
    );
    t.push(&AudioFrame {
        start_sample: 20000,
        pcm16: vec![0; 32000].into(),
    })
    .unwrap();
    assert!(t.clip(SampleSpan::new(15000, 21000).unwrap()).is_err());
    assert!(
        t.push(&AudioFrame {
            start_sample: u64::MAX,
            pcm16: vec![0; 2].into()
        })
        .is_err()
    );
    assert!(SampleSpan::from_seconds(f64::NAN, 1.).is_err());
}
#[test]
fn mixed_or_overlapping_speakers_never_receive_one_identity() {
    let span = SampleSpan::new(0, 32000).unwrap();
    let mut segments = vec![DiarizationSegment {
        label: label("0"),
        span,
        overlap: false,
    }];
    assert_eq!(label_span(span, &segments), Some(label("0")));
    segments.push(DiarizationSegment {
        label: label("1"),
        span: SampleSpan::new(10000, 11000).unwrap(),
        overlap: false,
    });
    assert_eq!(label_span(span, &segments), None);
    segments.pop();
    segments[0].overlap = true;
    assert_eq!(label_span(span, &segments), None);
}
struct Embed;
#[async_trait]
impl SpeakerEmbedder for Embed {
    fn model_id(&self) -> &str {
        "test-v1"
    }
    async fn embed(&self, _: &SpeakerAudio) -> Result<Vec<f32>> {
        Ok(vec![1., 0.])
    }
}
fn audio(seconds: usize) -> SpeakerAudio {
    SpeakerAudio {
        samples: (0..seconds * 16000)
            .map(|i| if i % 2 == 0 { 0.1 } else { -0.1 })
            .collect(),
    }
}
fn turn(id: &str, start: u64, seconds: usize) -> IdentityTurn {
    IdentityTurn {
        utterance_id: id.into(),
        source_session: "source".into(),
        span: SampleSpan::new(start, start + seconds as u64 * 16000).unwrap(),
        label: Some(label("0")),
        audio: audio(seconds),
    }
}
#[tokio::test]
async fn enrollment_is_explicit_and_survives_reload_then_forget() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(FileProfileStore::new(dir.path(), "space-a"));
    let recognizer = Arc::new(EmbeddingRecognizer {
        embedder: Arc::new(Embed),
    });
    let mut session =
        IdentitySession::new(recognizer.clone(), store.clone(), FusionConfig::default())
            .await
            .unwrap();
    let event = session.resolve(turn("cold", 0, 8)).await.unwrap();
    assert_eq!(event.resolution.user_id, None);
    assert!(session.profiles().is_empty());
    session
        .confirm_enrollment(&event.enrollment_candidate.unwrap(), "alice")
        .await
        .unwrap();
    assert!(session.hint("t", "outsider").is_err());
    let mut loaded = IdentitySession::new(recognizer, store.clone(), FusionConfig::default())
        .await
        .unwrap();
    assert_eq!(loaded.profiles().len(), 1);
    assert_eq!(
        loaded
            .resolve(turn("1", 0, 2))
            .await
            .unwrap()
            .resolution
            .confidence,
        IdentityConfidence::Medium
    );
    assert_eq!(
        loaded
            .resolve(turn("2", 32000, 2))
            .await
            .unwrap()
            .resolution
            .user_id
            .as_deref(),
        Some("alice")
    );
    assert!(loaded.resolve(turn("2", 32000, 2)).await.is_err());
    loaded.forget("alice").await.unwrap();
    assert!(store.load().await.unwrap().is_empty());
    assert!(
        FileProfileStore::new(dir.path(), "space-b")
            .load()
            .await
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn pipeline_submission_is_bounded_and_cancelled_on_drop() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(FileProfileStore::new(dir.path(), "a"));
    let identity = IdentitySession::new(
        Arc::new(EmbeddingRecognizer {
            embedder: Arc::new(Embed),
        }),
        store,
        FusionConfig::default(),
    )
    .await
    .unwrap();
    let (pipeline, mut output) = SpeakerPipeline::start(identity, None).unwrap();
    for i in 0..2 {
        pipeline
            .push_audio(AudioFrame {
                start_sample: i * 16000,
                pcm16: vec![10; 32000].into(),
            })
            .unwrap();
    }
    for i in 0..8 {
        pipeline
            .submit(TranscriptTurn {
                utterance_id: i.to_string(),
                source_session: "s".into(),
                span: SampleSpan::new(0, 32000).unwrap(),
                label: Some(label("0")),
            })
            .unwrap();
    }
    assert!(
        pipeline
            .submit(TranscriptTurn {
                utterance_id: "overflow".into(),
                source_session: "s".into(),
                span: SampleSpan::new(0, 32000).unwrap(),
                label: None
            })
            .is_err()
    );
    drop(pipeline);
    tokio::time::timeout(Duration::from_secs(1), async {
        while output.recv().await.is_some() {}
    })
    .await
    .unwrap();
}

#[test]
fn split_multi_speaker_transcript_preserves_unknown_overlap() {
    let segments = vec![
        DiarizationSegment {
            label: label("a"),
            span: SampleSpan {
                start: 0,
                end: 32000,
            },
            overlap: false,
        },
        DiarizationSegment {
            label: label("b"),
            span: SampleSpan {
                start: 32000,
                end: 64000,
            },
            overlap: false,
        },
    ];
    let parts = split_speaker_span(
        SampleSpan {
            start: 0,
            end: 64000,
        },
        &segments,
    );
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].1, Some(label("a")));
    assert_eq!(parts[1].1, Some(label("b")));
}

#[tokio::test]
async fn unlabeled_turn_does_not_hide_diarizer_session_change() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = IdentitySession::new(
        Arc::new(EmbeddingRecognizer {
            embedder: Arc::new(Embed),
        }),
        Arc::new(FileProfileStore::new(dir.path(), "session-reset")),
        FusionConfig::default(),
    )
    .await
    .unwrap();
    assert!(
        session
            .resolve(turn("old", 0, 4))
            .await
            .unwrap()
            .enrollment_candidate
            .is_none()
    );
    let mut unlabeled = turn("gap", 64000, 1);
    unlabeled.label = None;
    session.resolve(unlabeled).await.unwrap();
    let mut reconnected = turn("new", 80000, 4);
    reconnected.label.as_mut().unwrap().session = "new-connection".into();
    assert!(
        session
            .resolve(reconnected)
            .await
            .unwrap()
            .enrollment_candidate
            .is_none(),
        "enrollment audio must not accumulate across diarizer connections"
    );
}

#[tokio::test]
async fn rejected_pipeline_turn_cannot_clear_replay_protection() {
    let dir = tempfile::tempdir().unwrap();
    let mut identity = IdentitySession::new(
        Arc::new(EmbeddingRecognizer {
            embedder: Arc::new(Embed),
        }),
        Arc::new(FileProfileStore::new(dir.path(), "replay")),
        FusionConfig::default(),
    )
    .await
    .unwrap();
    identity.enroll_sample("alice", &audio(8)).await.unwrap();
    let (pipeline, mut output) = SpeakerPipeline::start(identity, None).unwrap();
    for start in [0, 16000] {
        pipeline
            .push_audio(AudioFrame {
                start_sample: start,
                pcm16: vec![10; 32000].into(),
            })
            .unwrap();
    }
    for attempt in 0..3 {
        pipeline
            .submit(TranscriptTurn {
                utterance_id: "same-turn".into(),
                source_session: "source".into(),
                span: SampleSpan::new(0, 32000).unwrap(),
                label: Some(label("0")),
            })
            .unwrap();
        let event = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Ok(event) = output.recv().await.unwrap() {
                    break event;
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(
            event.resolution.confidence,
            if attempt == 0 {
                IdentityConfidence::Medium
            } else {
                IdentityConfidence::Unknown
            }
        );
    }
}

use crate::*;
use anyhow::{Result, anyhow, ensure};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{Notify, mpsc, oneshot};

/// Final ASR coordinates must refer to the exact PCM16 stream passed to push_audio.
#[derive(Clone, Debug)]
pub struct TranscriptTurn {
    pub utterance_id: String,
    pub source_session: String,
    pub span: SampleSpan,
    pub label: Option<SpeakerLabel>,
}
struct AudioState {
    timeline: AudioTimeline,
    segments: VecDeque<DiarizationSegment>,
    live: Option<(mpsc::Sender<AudioFrame>, CancellationToken)>,
    generation: u64,
}
struct Lifetime(CancellationToken);
impl Drop for Lifetime {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
/// Cloneable host handle. Audio and transcript submission never wait for inference.
/// The last handle's drop cancels inference, diarization and queued work.
#[derive(Clone)]
pub struct SpeakerPipeline {
    commands: mpsc::Sender<Command>,
    state: Arc<Mutex<AudioState>>,
    changed: Arc<Notify>,
    diarizer: Option<Arc<dyn StreamingDiarizer>>,
    lifetime: Arc<Lifetime>,
}
enum Command {
    Turn(TranscriptTurn, SpeakerAudio),
    Proximity(String, ProximityBucket),
    Hint(String, String),
    Confirm(String, String, oneshot::Sender<Result<()>>),
    Enroll(String, SpeakerAudio, oneshot::Sender<Result<()>>),
    Forget(String, oneshot::Sender<Result<()>>),
    Reset,
}
impl SpeakerPipeline {
    pub fn start(
        mut identity: IdentitySession,
        diarizer: Option<Arc<dyn StreamingDiarizer>>,
    ) -> Result<(Self, mpsc::Receiver<Result<IdentityEvent>>)> {
        let (commands, mut rx) = mpsc::channel(8);
        let (events, output) = mpsc::channel(32);
        let state = Arc::new(Mutex::new(AudioState {
            timeline: AudioTimeline::new(60)?,
            segments: VecDeque::new(),
            live: None,
            generation: 0,
        }));
        let changed = Arc::new(Notify::new());
        let cancel = CancellationToken::new();
        let (worker_state, worker_changed, worker_cancel) =
            (state.clone(), changed.clone(), cancel.clone());
        let external_diarizer = diarizer.is_some();
        tokio::spawn(async move {
            loop {
                let command = tokio::select! { _ = worker_cancel.cancelled() => break, v = rx.recv() => match v { Some(v) => v, None => break } };
                let work = async {
                    match command {
                        Command::Turn(mut turn, audio) => {
                            if external_diarizer {
                                // Live-1 may finish a turn after the ASR final. This wait
                                // runs only in the identity worker; text has already gone out.
                                let join = async {
                                    loop {
                                        let notified = worker_changed.notified();
                                        let label = {
                                            let state = worker_state.lock().unwrap();
                                            label_span(
                                                turn.span,
                                                &state.segments.iter().cloned().collect::<Vec<_>>(),
                                            )
                                        };
                                        if label.is_some() {
                                            return label;
                                        }
                                        notified.await;
                                    }
                                };
                                turn.label = tokio::time::timeout(Duration::from_secs(3), join)
                                    .await
                                    .unwrap_or(None);
                            }
                            let parts = if external_diarizer {
                                let state = worker_state.lock().unwrap();
                                split_speaker_span(
                                    turn.span,
                                    &state.segments.iter().cloned().collect::<Vec<_>>(),
                                )
                            } else {
                                vec![(turn.span, turn.label.clone())]
                            };
                            for (span, label) in parts {
                                let clip = SpeakerAudio {
                                    samples: audio.samples[(span.start - turn.span.start) as usize
                                        ..(span.end - turn.span.start) as usize]
                                        .to_vec(),
                                };
                                let fallback = IdentityEvent {
                                    utterance_id: turn.utterance_id.clone(),
                                    source_session: turn.source_session.clone(),
                                    span,
                                    label: label.clone(),
                                    resolution: Resolution::unknown(),
                                    enrollment_candidate: None,
                                };
                                let result = tokio::time::timeout(
                                    Duration::from_secs(80),
                                    identity.resolve(IdentityTurn {
                                        utterance_id: turn.utterance_id.clone(),
                                        source_session: turn.source_session.clone(),
                                        span,
                                        label,
                                        audio: clip,
                                    }),
                                )
                                .await;
                                match result {
                                    Ok(Ok(event)) => {
                                        let _ = events.try_send(Ok(event));
                                    }
                                    _ => {
                                        identity.reset();
                                        let _ = events.try_send(Ok(fallback));
                                        let _ = events.try_send(Err(anyhow!(
                                            "speaker recognition failed or timed out"
                                        )));
                                    }
                                }
                            }
                        }
                        Command::Proximity(user, value) => identity.proximity(&user, value),
                        Command::Hint(turn, user) => {
                            let _ = identity.hint(&turn, &user);
                        }
                        Command::Confirm(candidate, user, reply) => {
                            let _ =
                                reply.send(identity.confirm_enrollment(&candidate, &user).await);
                        }
                        Command::Enroll(user, audio, reply) => {
                            let _ = reply.send(identity.enroll_sample(&user, &audio).await);
                        }
                        Command::Forget(user, reply) => {
                            let _ = reply.send(identity.forget(&user).await);
                        }
                        Command::Reset => identity.reset(),
                    }
                };
                tokio::select! { _ = worker_cancel.cancelled() => break, _ = tokio::time::timeout(Duration::from_secs(90), work) => {} }
            }
        });
        Ok((
            Self {
                commands,
                state,
                changed,
                diarizer,
                lifetime: Arc::new(Lifetime(cancel)),
            },
            output,
        ))
    }
    pub fn uses_external_diarizer(&self) -> bool {
        self.diarizer.is_some()
    }
    /// 16 kHz mono little-endian PCM16, maximum one second per frame.
    /// A discontinuity invalidates retained segments and starts a new Live-1 session.
    pub fn push_audio(&self, frame: AudioFrame) -> Result<()> {
        ensure!(
            !frame.pcm16.is_empty() && frame.pcm16.len() <= 32000,
            "audio frame must contain at most one second"
        );
        let mut state = self.state.lock().unwrap();
        let discontinuity = state.timeline.end_sample() != frame.start_sample;
        if discontinuity {
            if let Some((_, token)) = state.live.take() {
                token.cancel();
            }
            state.segments.clear();
            state.generation += 1;
            self.commands
                .try_send(Command::Reset)
                .map_err(|_| anyhow!("identity reset queue full"))?;
        }
        state.timeline.push(&frame)?;
        let Some(diarizer) = &self.diarizer else {
            return Ok(());
        };
        if state.live.as_ref().is_some_and(|(tx, _)| tx.is_closed()) {
            state.live = None;
        }
        if state.live.is_none() {
            state.generation += 1;
            let generation = state.generation;
            let (tx, rx) = mpsc::channel(200);
            let (segments, mut results) = mpsc::channel(32);
            let token = self.lifetime.0.child_token();
            state.live = Some((tx, token.clone()));
            let (shared, changed, provider) =
                (self.state.clone(), self.changed.clone(), diarizer.clone());
            tokio::spawn(async move {
                let run = provider.run(rx, segments, token.clone());
                tokio::pin!(run);
                loop {
                    tokio::select! {
                        _ = token.cancelled() => {
                            let _ = tokio::time::timeout(Duration::from_secs(5), &mut run).await;
                            break;
                        },
                        _ = &mut run => {
                            let mut state = shared.lock().unwrap();
                            if state.generation == generation {
                                while let Ok(Ok(segment)) = results.try_recv() {
                                    while state.segments.len() >= 256 { state.segments.pop_front(); }
                                    state.segments.push_back(segment);
                                }
                                changed.notify_one();
                            }
                            break;
                        },
                        v = results.recv() => match v {
                            Some(Ok(segment)) => {
                                let mut state = shared.lock().unwrap();
                                if state.generation != generation { break; }
                                while state.segments.len() >= 256 { state.segments.pop_front(); }
                                state.segments.push_back(segment); changed.notify_one();
                            },
                            Some(Err(_)) | None => break,
                        }
                    }
                }
                // Invalidate the sender so the next frame starts a fresh session.
                let mut state = shared.lock().unwrap();
                if state.generation == generation {
                    state.live = None;
                }
            });
        }
        let (tx, token) = state.live.as_ref().unwrap();
        if tx.try_send(frame).is_err() {
            token.cancel();
            state.live = None;
            state.generation += 1;
            return Err(anyhow!(
                "Live-1 audio queue full; starting a new session on next frame"
            ));
        }
        Ok(())
    }
    pub fn submit(&self, turn: TranscriptTurn) -> Result<()> {
        let audio = self.state.lock().unwrap().timeline.clip(turn.span)?;
        self.commands
            .try_send(Command::Turn(turn, audio))
            .map_err(|_| anyhow!("identity queue full or closed"))
    }
    pub fn proximity(&self, user: String, value: ProximityBucket) -> Result<()> {
        self.commands
            .try_send(Command::Proximity(user, value))
            .map_err(|_| anyhow!("identity queue full or closed"))
    }
    /// Hints are bounded corroboration for one pending utterance, never enrollment.
    pub fn hint(&self, utterance: String, user: String) -> Result<()> {
        self.commands
            .try_send(Command::Hint(utterance, user))
            .map_err(|_| anyhow!("identity queue full or closed"))
    }
    pub async fn confirm_enrollment(&self, candidate: String, user: String) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(Command::Confirm(candidate, user, tx))
            .await?;
        rx.await?
    }
    pub async fn enroll_sample(&self, user: String, audio: SpeakerAudio) -> Result<()> {
        audio.validate()?;
        let (tx, rx) = oneshot::channel();
        self.commands.send(Command::Enroll(user, audio, tx)).await?;
        rx.await?
    }
    pub async fn forget(&self, user: String) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.commands.send(Command::Forget(user, tx)).await?;
        rx.await?
    }
}

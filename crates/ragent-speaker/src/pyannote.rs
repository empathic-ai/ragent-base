use crate::*;
use anyhow::{Context, Result, anyhow, bail, ensure};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::mpsc,
    time::{Instant, timeout},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Does not expose keys or signed URLs through Debug or error messages.
#[derive(Clone)]
pub struct PyannoteClient {
    key: Arc<str>,
    http: reqwest::Client,
    base: String,
}
impl PyannoteClient {
    pub fn new(key: impl Into<String>) -> Result<Self> {
        let key = key.into();
        ensure!(!key.trim().is_empty(), "empty pyannote API key");
        Ok(Self {
            key: key.into(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            base: "https://api.pyannote.ai/v1".into(),
        })
    }
    pub fn from_env() -> Result<Self> {
        Self::new(std::env::var("PYANNOTE_API_KEY").context("Missing PYANNOTE_API_KEY")?)
    }
    async fn post<T: DeserializeOwned>(&self, path: &str, payload: &Value) -> Result<T> {
        let response = self
            .http
            .post(format!("{}{path}", self.base))
            .bearer_auth(&*self.key)
            .json(payload)
            .send()
            .await
            .map_err(|e| e.without_url())?;
        ensure!(
            response.status().is_success(),
            "pyannote {path} HTTP {}",
            response.status()
        );
        Ok(response.json().await.map_err(|e| e.without_url())?)
    }
    async fn upload(&self, audio: &SpeakerAudio) -> Result<String> {
        let wav = audio.wav()?;
        let media = format!("media://ragent/{}.wav", uuid::Uuid::new_v4());
        let upload: UrlResponse = self.post("/media/input", &json!({"url":media})).await?;
        let url = reqwest::Url::parse(&upload.url).map_err(|_| anyhow!("invalid upload URL"))?;
        ensure!(url.scheme() == "https", "insecure upload URL");
        let response = self
            .http
            .put(url)
            .header("Content-Type", "audio/wav")
            .body(wav)
            .send()
            .await
            .map_err(|e| e.without_url())?;
        ensure!(
            response.status().is_success(),
            "pyannote upload HTTP {}",
            response.status()
        );
        Ok(media)
    }
    async fn job(&self, path: &str, payload: Value) -> Result<Value> {
        // Submissions are not retried: ambiguous POST errors may already be billed.
        let created: JobCreated = self.post(path, &payload).await?;
        ensure!(
            created
                .job_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "invalid job id"
        );
        timeout(Duration::from_secs(60), async {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let response = self
                    .http
                    .get(format!("{}/jobs/{}", self.base, created.job_id))
                    .bearer_auth(&*self.key)
                    .send()
                    .await
                    .map_err(|e| e.without_url())?;
                if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
                ensure!(
                    response.status().is_success(),
                    "pyannote job polling HTTP {}",
                    response.status()
                );
                let job: Job = response.json().await.map_err(|e| e.without_url())?;
                match job.status.as_str() {
                    "succeeded" => return job.output.ok_or_else(|| anyhow!("missing job output")),
                    "failed" | "canceled" => bail!("pyannote job {}", job.status),
                    "created" | "pending" | "running" => {}
                    _ => bail!("unrecognized pyannote job status"),
                }
            }
        })
        .await
        .context("pyannote job deadline exceeded")?
    }
}
#[derive(Deserialize)]
struct UrlResponse {
    url: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobCreated {
    job_id: String,
}
#[derive(Deserialize)]
struct Job {
    status: String,
    output: Option<Value>,
}

/// Opaque voiceprints are compared by the remote identify job. Scores remain
/// provider evidence (0–100 scaled to 0–1), never assumed calibrated probabilities.
pub struct PyannoteVoiceprints {
    client: PyannoteClient,
    permit: Arc<tokio::sync::Semaphore>,
}
impl PyannoteVoiceprints {
    pub const MODEL_ID: &'static str = "pyannote/precision-2";
    pub fn new(client: PyannoteClient) -> Self {
        Self {
            client,
            permit: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }
}
#[async_trait]
impl SpeakerRecognizer for PyannoteVoiceprints {
    fn model_id(&self) -> &str {
        Self::MODEL_ID
    }
    async fn enroll(&self, audio: &SpeakerAudio) -> Result<VoiceTemplate> {
        audio.validate()?;
        ensure!(
            (8.0..=30.0).contains(&audio.duration_seconds()),
            "voiceprint enrollment requires 8–30 seconds"
        );
        let _permit = self.permit.acquire().await?;
        let url = self.client.upload(audio).await?;
        let output = self
            .client
            .job("/voiceprint", json!({"url":url,"model":"precision-2"}))
            .await?;
        let voiceprint = output
            .get("voiceprint")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("missing voiceprint"))?;
        Ok(VoiceTemplate::Opaque(voiceprint.into()))
    }
    async fn recognize(
        &self,
        audio: &SpeakerAudio,
        profiles: &[VoiceProfile],
    ) -> Result<VoiceEvidence> {
        audio.validate()?;
        validate_profiles(profiles)?;
        let voiceprints = profiles
            .iter()
            .filter(|p| p.model_id == self.model_id())
            .filter_map(|p| {
                if let VoiceTemplate::Opaque(v) = &p.template {
                    Some(json!({"label":p.user_id,"voiceprint":v}))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        if voiceprints.is_empty() {
            return Ok(VoiceEvidence {
                model_id: self.model_id().into(),
                scores: vec![],
                embedding: None,
            });
        }
        let _permit = self.permit.acquire().await?;
        let url = self.client.upload(audio).await?;
        let output=self.client.job("/identify",json!({"url":url,"model":"precision-2","voiceprints":voiceprints,"matching":{"threshold":0,"exclusive":false}})).await?;
        let scores = parse_identification(&output, profiles)?;
        Ok(VoiceEvidence {
            model_id: self.model_id().into(),
            scores,
            embedding: None,
        })
    }
}
fn parse_identification(output: &Value, profiles: &[VoiceProfile]) -> Result<Vec<VoiceScore>> {
    let rows = output
        .get("voiceprints")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing identification scores"))?;
    // A clip unexpectedly containing several speakers is unsafe for one identity.
    ensure!(
        rows.len() == 1,
        "identification clip contains zero or multiple speakers"
    );
    let confidence = rows[0]
        .get("confidence")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("missing confidence map"))?;
    profiles
        .iter()
        .filter(|p| p.model_id == PyannoteVoiceprints::MODEL_ID)
        .map(|p| {
            let score = confidence
                .get(&p.user_id)
                .and_then(Value::as_f64)
                .ok_or_else(|| anyhow!("missing candidate score"))?;
            ensure!(
                score.is_finite() && (0.0..=100.).contains(&score),
                "invalid candidate score"
            );
            Ok(VoiceScore {
                user_id: p.user_id.clone(),
                score: score as f32 / 100.,
            })
        })
        .collect()
}

#[derive(Clone)]
pub struct Live1Diarizer {
    client: PyannoteClient,
}
impl Live1Diarizer {
    pub fn new(client: PyannoteClient) -> Self {
        Self { client }
    }
}
#[derive(Deserialize)]
struct LiveSession {
    id: String,
    url: String,
}
#[derive(Deserialize)]
struct LiveData {
    timestamp: f64,
    speaker: String,
}
#[derive(Deserialize)]
#[serde(tag = "type")]
enum LiveEvent {
    #[serde(rename = "diarization_speaker_start")]
    Start { data: LiveData },
    #[serde(rename = "diarization_speaker_end")]
    End { data: LiveData },
    #[serde(rename = "error")]
    Error,
    #[serde(other)]
    Other,
}
#[derive(Default)]
struct LiveTurns {
    open: HashMap<String, (u64, bool)>,
}
impl LiveTurns {
    fn event(
        &mut self,
        text: &str,
        session: &str,
        origin: u64,
        sent: u64,
    ) -> Result<Option<DiarizationSegment>> {
        match serde_json::from_str::<LiveEvent>(text)? {
            LiveEvent::Start { data } => {
                let at = live_timestamp(data.timestamp, origin, sent)?;
                ensure!(
                    self.open.len() < 8 && !self.open.contains_key(&data.speaker),
                    "invalid Live-1 start sequence"
                );
                let overlap = !self.open.is_empty();
                for (_, tainted) in self.open.values_mut() {
                    *tainted = true;
                }
                self.open.insert(data.speaker, (at, overlap));
                Ok(None)
            }
            LiveEvent::End { data } => {
                let end = live_timestamp(data.timestamp, origin, sent)?;
                let (start, overlap) = self
                    .open
                    .remove(&data.speaker)
                    .ok_or_else(|| anyhow!("Live-1 end without start"))?;
                Ok(Some(DiarizationSegment {
                    label: SpeakerLabel {
                        provider: "pyannote-live-1".into(),
                        session: session.into(),
                        local: data.speaker,
                    },
                    span: SampleSpan::new(start, end)?,
                    overlap,
                }))
            }
            LiveEvent::Error => bail!("Live-1 reported a protocol error"),
            LiveEvent::Other => Ok(None),
        }
    }
}
fn live_timestamp(value: f64, origin: u64, sent: u64) -> Result<u64> {
    ensure!(
        value.is_finite() && value >= 0. && value * SAMPLE_RATE as f64 <= sent as f64 + 1.,
        "invalid Live-1 timestamp"
    );
    Ok(origin + (value * SAMPLE_RATE as f64).round() as u64)
}
#[async_trait]
impl StreamingDiarizer for Live1Diarizer {
    async fn run(
        &self,
        mut audio: mpsc::Receiver<AudioFrame>,
        events: mpsc::Sender<Result<DiarizationSegment>>,
        cancel: CancellationToken,
    ) -> Result<()> {
        let first = tokio::select! {_ = cancel.cancelled()=>return Ok(()),v=audio.recv()=>match v{Some(v)=>v,None=>return Ok(())}};
        ensure!(
            !first.pcm16.is_empty()
                && first.pcm16.len() <= 32000
                && first.pcm16.len().is_multiple_of(2),
            "invalid Live-1 first frame"
        );
        let live_request = json!({});
        let session: LiveSession = tokio::select! {_ = cancel.cancelled()=>return Ok(()),v=self.client.post("/live",&live_request)=>v?};
        ensure!(
            session.url.starts_with("wss://"),
            "insecure Live-1 websocket URL"
        );
        let socket = tokio::select! {_ = cancel.cancelled()=>return Ok(()),v=timeout(Duration::from_secs(20),connect_async(&session.url))=>v.context("Live-1 connection timeout")?.map_err(|_|anyhow!("Live-1 websocket connection failed"))?.0};
        run_live_socket(socket, first, audio, events, cancel, &session.id).await
    }
}
async fn run_live_socket<S>(
    socket: tokio_tungstenite::WebSocketStream<S>,
    first: AudioFrame,
    mut audio: mpsc::Receiver<AudioFrame>,
    events: mpsc::Sender<Result<DiarizationSegment>>,
    cancel: CancellationToken,
    session_id: &str,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send,
{
    let (mut sink, mut source) = socket.split();
    let origin = first.start_sample;
    let mut expected = first.end_sample();
    let mut buffered = VecDeque::from(first.pcm16.to_vec());
    let mut sent = 0u64;
    let mut valid_sent = 0u64;
    let mut turns = LiveTurns::default();
    let mut pace = tokio::time::interval(Duration::from_millis(100));
    pace.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut idle = Instant::now() + Duration::from_secs(4);
    let mut ending = false;
    let mut drain_deadline = None;
    loop {
        tokio::select! {
            _=cancel.cancelled(),if !ending=>{ending=true;},
            next=audio.recv(),if !ending && buffered.len()<16000*2*4=>{
                match next {Some(frame)=>{
                    ensure!(frame.start_sample==expected && frame.pcm16.len().is_multiple_of(2),"Live-1 source discontinuity; start a new session");
                    ensure!(frame.pcm16.len()<=16000*2,"Live-1 frame exceeds one second");
                    expected=frame.end_sample();buffered.extend(frame.pcm16);idle=Instant::now()+Duration::from_secs(4);
                },None=>ending=true}
            },
            _=pace.tick(),if drain_deadline.is_none()=>{
                if buffered.len()>=3200 || (ending && !buffered.is_empty()) {
                    let valid=buffered.len().min(3200);let mut chunk=vec![0u8;3200];
                    for v in chunk.iter_mut().take(valid){*v=buffered.pop_front().unwrap();}
                    let pcm=chunk.chunks_exact(2).flat_map(|v|(i16::from_le_bytes([v[0],v[1]]) as f32/32768.).to_le_bytes()).collect::<Vec<_>>();
                    timeout(Duration::from_secs(2),sink.send(Message::Binary(pcm))).await.context("Live-1 send timeout")?.map_err(|_|anyhow!("Live-1 audio send failed"))?;
                    sent+=1600;valid_sent+=valid as u64/2;
                }
                if ending && buffered.is_empty(){
                    timeout(Duration::from_secs(2),sink.send(Message::Text(r#"{"type":"end_of_stream"}"#.into()))).await.context("Live-1 finish timeout")?.map_err(|_|anyhow!("Live-1 finish failed"))?;
                    drain_deadline=Some(Instant::now()+Duration::from_secs(2));
                }
            },
            message=source.next()=>match message {
                Some(Ok(Message::Text(text)))=>{if let Some(mut segment)=turns.event(&text,session_id,origin,sent)?{segment.span.end=segment.span.end.min(origin+valid_sent);if segment.span.end<=segment.span.start{continue;}events.try_send(Ok(segment)).map_err(|_|anyhow!("Live-1 result queue full or closed"))?;}},
                Some(Ok(Message::Ping(data)))=>{timeout(Duration::from_secs(2),sink.send(Message::Pong(data))).await?.map_err(|_|anyhow!("Live-1 pong failed"))?;},
                Some(Ok(Message::Close(_)))|None=>{ensure!(ending,"Live-1 disconnected before input finished");return Ok(());},
                Some(Err(_))=>bail!("Live-1 receive failed"),_=>{}
            },
            _=tokio::time::sleep_until(drain_deadline.unwrap_or(idle)), if !ending || drain_deadline.is_some()=>{if ending{return Ok(());}ending=true;},
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_overlap_is_marked_and_session_scoped() {
        let mut p = LiveTurns::default();
        p.event(
            r#"{"type":"diarization_speaker_start","data":{"speaker":"S0","timestamp":0.0}}"#,
            "session",
            100,
            32000,
        )
        .unwrap();
        p.event(
            r#"{"type":"diarization_speaker_start","data":{"speaker":"S1","timestamp":0.5}}"#,
            "session",
            100,
            32000,
        )
        .unwrap();
        let e = p
            .event(
                r#"{"type":"diarization_speaker_end","data":{"speaker":"S0","timestamp":1.0}}"#,
                "session",
                100,
                32000,
            )
            .unwrap()
            .unwrap();
        assert!(e.overlap);
        assert_eq!(
            e.span,
            SampleSpan {
                start: 100,
                end: 16100
            }
        );
        assert_eq!(e.label.session, "session");
    }
}

#[cfg(test)]
mod protocol_tests {
    use super::*;
    #[test]
    fn scores_are_not_final_matches_and_multiple_speakers_abstain() {
        let profiles = vec![VoiceProfile {
            user_id: "alice".into(),
            model_id: PyannoteVoiceprints::MODEL_ID.into(),
            template: VoiceTemplate::Opaque("private".into()),
            sample_count: 1,
        }];
        let scores = parse_identification(
            &json!({"voiceprints":[{"match":null,"confidence":{"alice":86}}]}),
            &profiles,
        )
        .unwrap();
        assert!((scores[0].score - 0.86).abs() < 0.001);
        assert!(parse_identification(&json!({"voiceprints":[{},{}]}), &profiles).is_err());
        assert!(
            parse_identification(
                &json!({"voiceprints":[{"confidence":{"alice":101}}]}),
                &profiles
            )
            .is_err()
        );
    }
    #[tokio::test]
    async fn live_socket_paces_float_frames_and_flushes_partial_audio() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
            let mut frames = 0;
            let mut first = None;
            while let Some(message) = ws.next().await {
                match message.unwrap() {
                    Message::Binary(bytes) => {
                        assert_eq!(bytes.len(), 6400);
                        frames += 1;
                        if frames == 1 {
                            first = Some(Instant::now());
                            assert!(
                                (f32::from_le_bytes(bytes[0..4].try_into().unwrap()) - 0.5).abs()
                                    < 0.0001
                            );
                        } else {
                            assert!(first.unwrap().elapsed() >= Duration::from_millis(80));
                            assert_eq!(&bytes[400..404], &[0, 0, 0, 0]);
                        }
                    }
                    Message::Text(text) if text.contains("end_of_stream") => {
                        assert_eq!(frames, 2);
                        ws.send(Message::Text(r#"{"type":"diarization_speaker_start","data":{"speaker":"S0","timestamp":0.0}}"#.into())).await.unwrap();
                        ws.send(Message::Text(r#"{"type":"diarization_speaker_end","data":{"speaker":"S0","timestamp":0.2}}"#.into())).await.unwrap();
                        ws.close(None).await.unwrap();
                        return;
                    }
                    _ => {}
                }
            }
            panic!("missing end_of_stream");
        });
        let (socket, _) = connect_async(format!("ws://{address}")).await.unwrap();
        let (tx, rx) = mpsc::channel(2);
        drop(tx);
        let (events, mut output) = mpsc::channel(4);
        let first = AudioFrame {
            start_sample: 32000,
            pcm16: (0..1700)
                .flat_map(|_| 16384i16.to_le_bytes())
                .collect::<Vec<_>>()
                .into(),
        };
        timeout(
            Duration::from_secs(3),
            run_live_socket(
                socket,
                first,
                rx,
                events,
                CancellationToken::new(),
                "live-session",
            ),
        )
        .await
        .unwrap()
        .unwrap();
        let segment = output.recv().await.unwrap().unwrap();
        assert_eq!(
            segment.span,
            SampleSpan {
                start: 32000,
                end: 33700
            }
        );
        server.await.unwrap();
    }
}

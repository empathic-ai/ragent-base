# Speaker identification

The opt-in native-host pipeline keeps transcription, diarization and persistent
identity separate. A diarization label is scoped to its provider connection;
only `IdentitySession` resolves an enrolled user. Identity never authenticates
a network sender or changes a `SpeakEvent`'s original sender.

## Provider selection

| Concern | Options | Feature |
| --- | --- | --- |
| Transcription | Deepgram or an injected `Box<dyn Transcriber>` | `deepgram` for Deepgram |
| Diarization | Transcriber's final speaker labels or pyannote Live-1 | `pyannote` for Live-1 |
| Recognition | Polyvoice ResNet34 embeddings or pyannote Voiceprints | `polyvoice` / `pyannote` |
| Persistence | Host `VoiceProfileStore`; local atomic file implementation included | `speaker-identification` |

```rust,ignore
let options = SpeakerOptions::from_env(store).await?;
let asr = DeepgramTranscriber::new_from_env()
    .with_diarization(options.diarizer.is_none());
let space = SpaceWorker::new_with_speakers(space_id, Box::new(asr), options).await?;
```

`SpaceWorker::new` retains its existing behavior. The Empathic companion change
initializes the new constructor when `EMPATHIC_SPEAKER_IDENTITY=1`.

| Environment | Values |
| --- | --- |
| `SPEAKER_RECOGNIZER` | `polyvoice` (default), `pyannote` |
| `SPEAKER_DIARIZER` | `deepgram` (default), `live-1` |
| `POLYVOICE_MODEL_PATH` | Local ResNet34 INT8 ONNX weights for Polyvoice native inference |
| `PYANNOTE_API_KEY` | Required by either pyannote adapter |
| `DEEPGRAM_API_KEY` | Required by Deepgram transcription |
| `SPEAKER_FUSION_CONFIG` | Optional JSON matching `FusionConfig` |

Model files are supplied by the host. Their SHA-256 is part of the Polyvoice
profile's model ID. Polyvoice is pinned to 0.19.0: 0.20.0's native embedder failed
compilation against its published kernels dependency during implementation.

Pyannote Voiceprints are opaque server-matched templates, **not vectors**. They
cannot be averaged or cosine-compared with Polyvoice profiles. Both formats are
persisted with model IDs; switching models requires separate enrollment.
Identify/voiceprint use asynchronous upload, job submission and polling, so their
latency and billing differ from local embeddings. No credentials, signed URLs,
voiceprint blobs or raw audio are included in identity events.

## Audio and timing contract

Input is 16 kHz mono PCM16 little-endian. The worker records a bounded 60-second
identity history **before** sending the same bytes to ASR and Live-1. Inference
has an eight-item queue and never blocks transcript delivery. A slow identity
backend drops excess evidence; it does not guess who spoke.

A custom transcriber must provide `is_final`, finite `start_seconds` and
`end_seconds`, `session_id`, and `stream_start_sample`. Times are relative to
that connection; the sample offset refers to the exact input byte stream.
Omit the offset after unknown-duration audio loss. Untimed transcribers still
produce text, but cannot produce acoustic identities. The existing Whisper and
WebSpeech streaming implementations are unfinished; this change does not
implement those ASR backends.

Deepgram exposes final contiguous word-speaker groups, preserves punctuation,
and advances the source sample clock across reconnects. ASR connection IDs reset
identity continuity. Broadcast lag invalidates identity timing until that
transcriber is replaced. A source that drops/resamples audio before this worker
must expose a new source epoch; do not join timestamps from different clocks.

Live-1 receives paced 100 ms float32 frames converted from the same PCM16 audio.
Each connection has its own source origin and speaker namespace. Queue loss or
connection failure starts a fresh session on subsequent audio. Partial final
frames are padded for transport; annotations are clipped to real input samples.
Identity waits up to three seconds for finalized diarization, without delaying
text. Multi-speaker transcripts receive audio-range annotations. Overlap,
uncovered ranges, short/poor audio, expired history and ambiguous scores abstain.
At most eight pieces are considered per transcript; more fragmented results
remain unknown. These bounds favor precision over complete attribution.

## Fusion and enrollment

Voice scores are backend evidence, not calibrated probabilities. Every committed
identity must pass the current acoustic threshold and winner margin, a bounded
decaying fusion threshold, and consecutive evidence checks. Proximity, recent
acoustic continuity, session-local grouping and a single semantic hint can only
corroborate a voice-supported candidate. Measure thresholds on labeled recordings
from the deployment microphones before enabling identity-dependent behavior.

`SpeakerResolvedEvent` identifies an utterance and an audio sample range. Its
`user_id` is set only for high confidence; medium confidence has a separate
candidate. Agent context receives a new annotation without editing previous
messages or automatically starting another response.

Unknown embedding clips can accumulate in bounded, short-lived acoustic clusters.
After eight seconds they emit a private enrollment candidate ID. Voiceprints have
no exposed embedding, so their candidates require one qualifying eight-second
clip instead of trusting local diarization labels to accumulate different clips.
The host must confirm the person and single-speaker sample before enrolling:

```rust,ignore
pipeline.confirm_enrollment(candidate_id, user_id.to_string()).await?;
// Or a separately captured, confirmed 8–30 second single-speaker sample:
pipeline.enroll_sample(user_id.to_string(), audio).await?;
pipeline.forget(user_id.to_string()).await?;
pipeline.proximity(user_id.to_string(), ProximityBucket::from_rssi(rssi))?;
```

Enrollment and deletion persist before updating the active gallery. Only one
writer should own a scope. The local store uses atomic replacement and Unix
0700/0600 permissions; multi-process hosts should supply a transactional store.
Empathic's companion adapter uses Flux public record helpers with a private,
space-scoped SurrealDB gallery, separate from `User.voice_id` synthesis aliases.

`IdentitySession::set_embedding_adaptation(true)` enables conservative updates
only after high confidence, a stricter score/margin and outlier rejection. It is
off by default; opaque voiceprints always require explicit re-enrollment.

The optional `SpeakerHintEvent` tool must be offered with a host-scoped candidate
list. Hints reference a recent known utterance and can corroborate a subsequent
matching local-speaker segment; they cannot enroll or override acoustic gates.
BLE scanning/device-to-user association belongs to Empathic firmware/host code.
The pipeline accepts trusted, enrolled-user proximity updates with a 30-second
TTL; it does not infer a person from an arbitrary BLE address.

## Validation

Run the independent core suite without service credentials:

```sh
cargo test --manifest-path crates/ragent-speaker/Cargo.toml --locked --all-features
cargo test --manifest-path crates/ragent-speaker/Cargo.toml --locked --no-default-features
```

Tests cover identity switching/unknowns, ambiguity, invalid scores, time decay,
model/session isolation, audio gaps, multi-speaker ranges, overlap abstention,
explicit enrollment/reload/deletion, bounded submission/cancellation, pyannote
score parsing and actual local WebSocket framing/pacing/finalization.

The full Ragent build requires access to its private Git dependencies. The
implementation environment encountered HTTP 401 fetching `common`; isolated
core tests are not a full Bevy/Empathic build or hardware validation. Real model
accuracy, service credentials, noisy-room recordings, BLE hardware and the full
Empathic development build remain deployment validation gates.

## Provider references

- [Live-1 streaming contract](https://docs.pyannote.ai/tutorials/streaming-real-time)
- [Voiceprint creation and identification](https://docs.pyannote.ai/tutorials/identification-with-voiceprints)
- [Polyvoice source](https://github.com/ekhodzitsky/polyvoice)

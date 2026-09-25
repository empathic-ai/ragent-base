# Ragent

Ragent owns chat, transcription, synthesis, voice aliases, and provider settings.
Applications and host tools use the `ChatCompleter` and `Synthesizer` interfaces
from the prelude, rather than constructing provider requests themselves.

## Features and builds

`bevy`, `tokio`, and `futures` are the default integration features. Providers
such as `openai`, `deepgram`, `anthropic`, and `candle` are selected separately;
provider modules also respect their existing target gates. Candle includes the
local model, tokenizer, model-download, image-decoding, and Chrome-profiling
stack. Image and profiling dependencies are not compiled when Candle is disabled.

`prost` enables Protobuf generation; `tonic` adds gRPC generation. The generators
are optional **host** dependencies, so ordinary provider and ESP builds do not
compile them. Build scripts track the protocol directory and propagate generator
errors rather than leaving stale or missing output unnoticed.

## Local AI backends

The `local-ai` feature enables `SherpaTranscriber`, `SherpaSynthesizer`, and
`LlamaCppChatCompleter`. The individual `sherpa` and `llama-cpp` features can
be enabled when only part of the stack is needed. These modules are excluded
from WASM and XTensa builds.

```rust,ignore
use ragent::prelude::*;

let stt = SherpaTranscriber::from_streaming_zipformer(
	"models/stt/encoder.onnx", "models/stt/decoder.onnx",
	"models/stt/joiner.onnx", "models/stt/tokens.txt", 16_000,
)?;
let tts = SherpaSynthesizer::new(sherpa_onnx::OfflineTtsConfig { /* model paths */ ..Default::default() })?;
let llm = LlamaCppChatCompleter::from_file("models/llm/model-q4.gguf", Default::default())?;
```

Sherpa expects external ONNX model files and tokens. Streaming ASR consumes
little-endian mono PCM16 chunks; TTS returns a PCM16 WAV in `SynthesisResult`.
Preset speakers use `SherpaVoiceConfig::Preset`; Pocket/ZipVoice-style cloning
uses `SherpaVoiceConfig::Cloned` with reference PCM and optional transcript.
Local synthesis and completion report zero external cost.

The llama backend keeps the GGUF model loaded, applies the model's embedded chat
template, and streams token deltas through `ChatCompletionResponse`. Its context
and native generation state are confined to a blocking worker because
`llama-cpp-2` contexts are not `Send`.

`llama-cpp-2` uses the portable CPU path by default. Enable `llama-cpp-metal`
on Apple targets when the Metal toolchain is available; this is opt-in and CPU
fallback remains available. Enable `llama-cpp-android` for the crate's Android
shared C++ runtime configuration. APK packaging still needs the NDK-produced
native libraries supplied by the application. Sherpa's native
archive availability must likewise be checked for the selected Android/iOS
target; model assets are never bundled by Ragent.

Voice IDs such as `child-a` are application aliases. The provider mapping and
synthesis settings belong here. Moving an alias or changing synthesis settings
can affect offline voice packs; see the owning application and CLI guides.

## Historical experiments

`docs/legacy/lib-experiments.rs.txt` preserves the old event-construction code
verbatim. It is intentionally outside the active module tree. Restoring it needs
review against the current reflection and task contracts; it is not a supported
feature solely because its source is retained.

`docs/legacy/build.rs.txt` also retains the original protocol build script,
including its explicit output-directory setup and generator branching.

## Speaker identification

See [speaker identification](docs/speaker-identification.md) for the opt-in
native pipeline, provider configuration, audio contracts, enrollment, testing,
and offline evaluation workflow.


## Logging

See the workspace [logging guide](../../docs/logging.md) for selectable groups,
local configuration, VS Code controls, and platform-specific behavior.

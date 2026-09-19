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


## Logging

See the workspace [logging guide](../../docs/logging.md) for selectable groups,
local configuration, VS Code controls, and platform-specific behavior.

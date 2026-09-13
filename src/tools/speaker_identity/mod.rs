//! Speaker identification integration and provider-neutral core.
//! See `docs/speaker-identification.md` for configuration and lifecycle.
pub use ragent_speaker::*;
mod worker;
pub use worker::*;

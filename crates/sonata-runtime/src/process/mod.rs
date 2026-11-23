//! Linux subprocess runners for JS cipher evaluation and `yt-dlp` stream resolution.
//!
//! 1. **CLI Cipher Bridge** (`cipher`): Evaluates hooked YouTube `base.js` cipher scripts via
//!    local `node`, `bun`, or `deno` binaries behind `sonata_core::providers::hifi::JsCipherBridge`.
//! 2. **Stream Resolver Composition** (`ytdlp`): `LinuxStreamResolver` composes portable
//!    `sonata_core::providers::hifi::StreamResolver` with `CliJsCipherBridge` and `yt-dlp`
//!    fallback / fast-start racing.

pub mod cipher;
pub mod ytdlp;

pub use cipher::CliJsCipherBridge;
pub use ytdlp::{
    detect_js_runtime, parse_ytdlp_audio_line, LinuxStreamResolver, YtdlpAudioMetadata,
};

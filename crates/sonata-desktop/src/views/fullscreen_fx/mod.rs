//! High-fidelity visual effects for the immersive fullscreen playback workspace,
//! implemented on top of stock GPUI (`RenderImage` + `cosmic-text`) without a custom GPUI fork.
//!
//! 1. **Cover Palette Extraction (`palette`)**: 24-bin circular hue clustering yielding a leading hue, a distinct secondary runner-up hue, and darkened stage colors.
//! 2. **5-Color Flowing Ambient Waves (`waves`)**: pure-Rust CPU wave-field renderer emitting compact `RenderImage` frames with exponential 0.8s color-wash transitions.
//! 3. **Gaussian Depth-of-Field Blurred Lyrics (`blur_cache`)**: `cosmic-text` text shaping/rasterization with alpha-invariant 3-pass separable box blur and `Inter` font matching.

mod blur_cache;
mod palette;
mod waves;

pub use blur_cache::BlurredLyricCache;
pub use waves::AmbientWavesState;

//! Shared desktop UI utilities: bounded DPR-aware media/blur cache and floating-glass GPUI widgets.
//!
//! 1. **Media (`media`)**: `24 MB` bounded LRU `RenderImage` cache, async image decoding, `96x96` box-blurred wash generator, and dominant color extraction.
//! 2. **Components (`components`)**: Floating cards, glass buttons, delayed (`150 ms`) `Skeleton` shimmers, hover `play_overlay`, `LoadingRing`, clickable multi-artist credits, `header_wash`, and true-alpha scroll edge fades.

pub mod components;
pub mod media;

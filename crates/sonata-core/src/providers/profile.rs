//! Explicit stream-resolution policy types replacing compile-time platform branches.
//!
//! 1. **Policy Modes**: `FastStartMode`, `UpgradeDelivery`, and `StreamFallbackMode` express
//!    mutually exclusive stream bootstrap, upgrade, and fallback behaviors as typed enums.
//! 2. **Profile Container**: `ResolverProfile` bundles per-engine stream resolution policy
//!    (fast-start mode, upgrade delivery, fallback mode, bootstrap timeout, JioSaavn toggle, and
//!    cache byte limit).

use std::time::Duration;

/// Controls whether initial fast-start streams append a bounded byte range (`range=0-...`)
/// or return a direct stream URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FastStartMode {
    /// Request an initial 1 MiB byte-range fast-start URL (Linux desktop/TUI behavior).
    BoundedRange,
    /// Return a direct playable URL without appending a `range=0-...` query parameter (Android behavior).
    DirectUrl,
}

/// Controls how a resolved Hi-Fi upgrade candidate is delivered to playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeDelivery {
    /// Download and commit the upgraded stream to local disk cache before returning it.
    CacheBeforeReturn,
    /// Allow returning the remote upgraded stream directly when local caching fails or is skipped.
    ReturnRemoteStream,
}

/// Controls whether stream resolution and recovery may invoke external CLI fallbacks (`yt-dlp`)
/// or remain strictly inside direct InnerTube protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamFallbackMode {
    /// Resolve and recover strictly via portable InnerTube clients (Android behavior).
    InnerTubeOnly,
    /// Race InnerTube with external tools and fall back to external recovery (Linux behavior).
    ExternalTool,
}

/// Explicit per-engine stream resolution policy injected by the owning client.
///
/// # Examples
///
/// ```
/// use sonata_core::providers::profile::{
///     FastStartMode, ResolverProfile, StreamFallbackMode, UpgradeDelivery,
/// };
/// use std::time::Duration;
///
/// let profile = ResolverProfile {
///     fast_start: FastStartMode::DirectUrl,
///     upgrade_delivery: UpgradeDelivery::ReturnRemoteStream,
///     fallback: StreamFallbackMode::InnerTubeOnly,
///     bootstrap_timeout: Duration::from_millis(8500),
///     jiosaavn_enabled: false,
///     cache_limit_bytes: 2 * 1024 * 1024 * 1024,
/// };
/// assert!(!profile.uses_bounded_fast_start());
/// assert!(!profile.allows_external_fallback());
/// assert!(profile.cache_enabled());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverProfile {
    pub fast_start: FastStartMode,
    pub upgrade_delivery: UpgradeDelivery,
    pub fallback: StreamFallbackMode,
    pub bootstrap_timeout: Duration,
    pub jiosaavn_enabled: bool,
    pub cache_limit_bytes: u64,
}

impl ResolverProfile {
    /// Returns `true` when fast-start resolution should request bounded initial byte ranges.
    pub const fn uses_bounded_fast_start(&self) -> bool {
        matches!(self.fast_start, FastStartMode::BoundedRange)
    }

    /// Returns `true` when remote upgrade streams may be returned if local caching does not succeed.
    pub const fn allows_remote_upgrade_fallback(&self) -> bool {
        matches!(self.upgrade_delivery, UpgradeDelivery::ReturnRemoteStream)
    }

    /// Returns `true` when external CLI fallback (`yt-dlp`) is allowed for fast-start, upgrades,
    /// and recovery.
    pub const fn allows_external_fallback(&self) -> bool {
        matches!(self.fallback, StreamFallbackMode::ExternalTool)
    }

    /// Returns `true` when on-disk media/audio caching is enabled for this resolver.
    pub const fn cache_enabled(&self) -> bool {
        self.cache_limit_bytes > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_predicates_match_explicit_modes() {
        let linux = ResolverProfile {
            fast_start: FastStartMode::BoundedRange,
            upgrade_delivery: UpgradeDelivery::CacheBeforeReturn,
            fallback: StreamFallbackMode::ExternalTool,
            bootstrap_timeout: Duration::from_millis(2400),
            jiosaavn_enabled: true,
            cache_limit_bytes: 2 * 1024 * 1024 * 1024,
        };
        assert!(linux.uses_bounded_fast_start());
        assert!(!linux.allows_remote_upgrade_fallback());
        assert!(linux.allows_external_fallback());
        assert!(linux.cache_enabled());

        let android = ResolverProfile {
            fast_start: FastStartMode::DirectUrl,
            upgrade_delivery: UpgradeDelivery::ReturnRemoteStream,
            fallback: StreamFallbackMode::InnerTubeOnly,
            bootstrap_timeout: Duration::from_millis(8500),
            jiosaavn_enabled: false,
            cache_limit_bytes: 0,
        };
        assert!(!android.uses_bounded_fast_start());
        assert!(android.allows_remote_upgrade_fallback());
        assert!(!android.allows_external_fallback());
        assert!(!android.cache_enabled());
    }
}

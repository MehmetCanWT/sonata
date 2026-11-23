//! Linux desktop browser profile discovery and cookie database extraction.
//!
//! 1. **Browser Extraction (`browser`)**: Discovers installed Gecko and Chromium profiles,
//!    reads Firefox/Zen SQLite jars safely from private temp copies, and delegates Chromium
//!    cookie extraction to `yt-dlp`.

pub mod browser;

pub use browser::{
    extract_browser_cookies, extract_ytm_cookies_from_browser, list_supported_browsers,
    BrowserOption,
};

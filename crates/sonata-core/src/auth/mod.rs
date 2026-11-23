//! Portable authentication models, cookie signing, and private session persistence.
//!
//! 1. **Cookies (`cookies`)**: `ExtractedCookies`, `CookieRow`, Netscape cookie parsing,
//!    `SAPISIDHASH` request signing, and `0o600` session file persistence.

pub mod cookies;

pub use cookies::{
    build_from_rows, compute_sapisidhash, compute_sapisidhash_triple, compute_sapisidhash_variants,
    create_private_file, ensure_private_dir, parse_netscape_cookies, write_private_file, CookieRow,
    ExtractedCookies,
};

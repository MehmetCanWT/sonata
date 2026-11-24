//! Credential and signed-URL redaction helpers for Sonata logs and errors.
//!
//! 1. **URL Scrubbing**: `stream_url` strips query parameters, paths, and userinfo from HTTP(S) URLs.
//! 2. **Text & JSON Scrubbing**: `sensitive_text` and `json_payload` redact cookies, auth tokens,
//!    self-hosted server credentials (`X-Emby-Token`, `X-MediaBrowser-Token`, `api_key`, Subsonic
//!    `t`/`p`, `Basic ` auth), scrobbler credentials (`api_sig`, `sk`, `api_secret`, `Token `),
//!    and signed URLs.

use regex::{Captures, Regex};
use reqwest::Url;
use serde_json::{json, Value};
use std::sync::OnceLock;

const REDACTED_STREAM: &str = "[redacted-stream-url]";
const REDACTED_SECRET: &str = "[redacted-sensitive-value]";

const FULL_LINE_MARKERS: [&str; 23] = [
    "authorization:",
    "authorization=",
    "cookie:",
    "cookie=",
    "cookies=",
    "sapisid:",
    "sapisid=",
    "sp_dc:",
    "sp_dc=",
    "x-emby-token:",
    "x-emby-token=",
    "x-emby-authorization:",
    "x-emby-authorization=",
    "x-mediabrowser-token:",
    "x-mediabrowser-token=",
    "api_key:",
    "api_key=",
    "api_secret:",
    "api_secret=",
    "api_sig:",
    "api_sig=",
    "session_key:",
    "session_key=",
];

pub fn stream_url(input: &str) -> String {
    let Ok(url) = Url::parse(input) else {
        return "[local-media]".to_string();
    };
    if !matches!(url.scheme(), "http" | "https") {
        return "[local-media]".to_string();
    }
    let Some(host) = url.host_str() else {
        return REDACTED_STREAM.to_string();
    };
    let port = url
        .port()
        .map_or_else(String::new, |port| format!(":{port}"));
    format!("{}://{host}{port}/{REDACTED_STREAM}", url.scheme())
}

pub fn sensitive_text(text: &str) -> String {
    text.lines()
        .take(12)
        .map(redact_line)
        .collect::<Vec<_>>()
        .join(" | ")
}

fn basic_auth_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?i)\b(Basic|Token)\s+([^\s"';,]+)"#).unwrap())
}

fn named_secret_param_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?i)(^|[?&\s;,])(api_key|api_secret|api_sig|session_key|x-emby-token|x-mediabrowser-token)=([^&\s"';,]+)"#,
        )
        .unwrap()
    })
}

fn subsonic_param_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?i)(^|[?&\s;,])(t|p|sk)=([^&\s"';,]+)"#).unwrap())
}

fn redact_inline_tokens(input: &str) -> String {
    let pass1 = basic_auth_regex().replace_all(input, |caps: &Captures<'_>| {
        format!("{} {REDACTED_SECRET}", &caps[1])
    });
    let pass2 = named_secret_param_regex().replace_all(&pass1, |caps: &Captures<'_>| {
        format!("{}{}={REDACTED_SECRET}", &caps[1], &caps[2])
    });
    subsonic_param_regex()
        .replace_all(&pass2, |caps: &Captures<'_>| {
            format!("{}{}={REDACTED_SECRET}", &caps[1], &caps[2])
        })
        .into_owned()
}

fn contains_sensitive_marker(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    FULL_LINE_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
        || basic_auth_regex().is_match(text)
        || named_secret_param_regex().is_match(text)
        || subsonic_param_regex().is_match(text)
}

fn redact_line(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let secret_at = FULL_LINE_MARKERS
        .iter()
        .filter_map(|marker| lower.find(marker))
        .min();
    if let Some(position) = secret_at {
        return format!("{}{REDACTED_SECRET}", &line[..position]);
    }

    let mut output = String::new();
    for (index, part) in line.split_whitespace().enumerate() {
        if index > 0 {
            output.push(' ');
        }
        if let Some(position) = part.find("https://").or_else(|| part.find("http://")) {
            let prefix = redact_inline_tokens(&part[..position]);
            output.push_str(&prefix);
            output.push_str(&stream_url(&part[position..]));
        } else {
            output.push_str(&redact_inline_tokens(part));
        }
    }
    redact_inline_tokens(&output)
}

fn is_sensitive_json_key(lower: &str) -> bool {
    lower.contains("authorization")
        || lower.contains("cookie")
        || lower.contains("http-header")
        || lower.contains("x-emby-token")
        || lower.contains("x-mediabrowser-token")
        || lower.contains("api_key")
        || lower.contains("api_secret")
        || lower.contains("api_sig")
        || lower.contains("session_key")
        || matches!(
            lower,
            "apikey"
                | "apisecret"
                | "apisig"
                | "access_token"
                | "accesstoken"
                | "bearer_token"
                | "password"
                | "token"
                | "sk"
                | "t"
                | "p"
        )
}

pub fn json_payload(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    let lower = key.to_ascii_lowercase();
                    let redacted = if matches!(lower.as_str(), "url" | "file") {
                        value
                            .as_str()
                            .map(stream_url)
                            .map_or_else(|| json_payload(value), |safe| json!(safe))
                    } else if is_sensitive_json_key(&lower) {
                        json!(REDACTED_SECRET)
                    } else {
                        json_payload(value)
                    };
                    (key.clone(), redacted)
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(json_payload).collect()),
        Value::String(text) if text.starts_with("http://") || text.starts_with("https://") => {
            json!(stream_url(text))
        }
        Value::String(text) if contains_sensitive_marker(text) => {
            json!(sensitive_text(text))
        }
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_signed_stream_url_without_leaking_credentials() {
        let secret = "token-secret";
        let safe = stream_url(&format!(
            "https://user:password@media.example:8443/audio?id=1&sig={secret}"
        ));
        assert_eq!(safe, "https://media.example:8443/[redacted-stream-url]");
        assert!(!safe.contains(secret));
        assert!(!safe.contains("password"));
    }

    #[test]
    fn redacts_cookie_and_authorization_values() {
        let safe = sensitive_text(
            "Cookie: SAPISID=cookie-secret\nAuthorization: Bearer auth-secret\nurl=https://x.test/a?sig=url-secret",
        );
        for secret in ["cookie-secret", "auth-secret", "url-secret"] {
            assert!(!safe.contains(secret));
        }
        assert!(safe.contains(REDACTED_SECRET));
        assert!(safe.contains(REDACTED_STREAM));
    }

    #[test]
    fn redacts_nested_mpv_headers_and_urls() {
        let safe = json_payload(&json!({
            "command": ["loadfile", "https://media.test/a?sig=url-secret"],
            "options": {"http-header-fields": "Cookie=cookie-secret,Authorization=auth-secret"}
        }));
        let rendered = safe.to_string();
        for secret in ["url-secret", "cookie-secret", "auth-secret"] {
            assert!(!rendered.contains(secret));
        }
    }

    #[test]
    fn redacts_self_hosted_server_tokens_params_and_basic_auth() {
        let samples = [
            "X-Emby-Token: jf-emby-secret",
            "X-MediaBrowser-Token: jf-mb-secret",
            "X-Emby-Authorization: MediaBrowser Client=\"Sonata\", Token=\"jf-hdr-secret\"",
            "GET /Items?api_key=jf-api-secret&Recursive=true",
            "GET /rest/stream.view?u=alice&t=sub-token-secret&s=salt123&p=sub-pass-secret&v=1.16.1",
            "Authorization: Basic dXNlcjpwYXNzLXNlY3JldA==",
            "header Basic dXNlcjpwYXNzLXNlY3JldA== rejected",
            "https://alice:dav-pass-secret@dav.example.com/music/track.flac",
        ];

        let secrets = [
            "jf-emby-secret",
            "jf-mb-secret",
            "jf-hdr-secret",
            "jf-api-secret",
            "sub-token-secret",
            "sub-pass-secret",
            "dXNlcjpwYXNzLXNlY3JldA==",
            "dav-pass-secret",
        ];

        for sample in samples {
            let safe = sensitive_text(sample);
            for secret in secrets {
                assert!(
                    !safe.contains(secret),
                    "sensitive_text leaked `{secret}` in `{safe}`"
                );
            }
        }

        let payload = json_payload(&json!({
            "headers": [
                "X-Emby-Token: jf-emby-secret",
                "X-MediaBrowser-Token: jf-mb-secret",
                "Authorization: Basic dXNlcjpwYXNzLXNlY3JldA=="
            ],
            "params": {
                "api_key": "jf-api-secret",
                "t": "sub-token-secret",
                "p": "sub-pass-secret",
                "u": "alice"
            }
        }));
        let rendered = payload.to_string();
        for secret in [
            "jf-emby-secret",
            "jf-mb-secret",
            "dXNlcjpwYXNzLXNlY3JldA==",
            "jf-api-secret",
            "sub-token-secret",
            "sub-pass-secret",
        ] {
            assert!(
                !rendered.contains(secret),
                "json_payload leaked `{secret}` in `{rendered}`"
            );
        }
        assert!(rendered.contains("alice"));
    }

    #[test]
    fn redacts_scrobble_secrets_and_listenbrainz_tokens() {
        let samples = [
            "POST method=track.scrobble&api_key=lfm-key-secret&sk=lfm-sk-secret&api_sig=lfm-sig-secret",
            "api_secret=lfm-app-secret session_key=lfm-sess-secret",
            "Authorization: Token lb-user-token-secret",
            "header Token lb-inline-token-secret rejected",
            "https://ws.audioscrobbler.com/2.0/?method=track.scrobble&api_key=lfm-url-key&api_sig=lfm-url-sig&sk=lfm-url-sk",
        ];
        let secrets = [
            "lfm-key-secret",
            "lfm-sk-secret",
            "lfm-sig-secret",
            "lfm-app-secret",
            "lfm-sess-secret",
            "lb-user-token-secret",
            "lb-inline-token-secret",
            "lfm-url-key",
            "lfm-url-sig",
            "lfm-url-sk",
        ];

        for sample in samples {
            let safe = sensitive_text(sample);
            for secret in secrets {
                assert!(
                    !safe.contains(secret),
                    "sensitive_text leaked `{secret}` in `{safe}`"
                );
            }
        }

        let payload = json_payload(&json!({
            "api_key": "lfm-json-key",
            "api_secret": "lfm-json-secret",
            "api_sig": "lfm-json-sig",
            "sk": "lfm-json-sk",
            "token": "lb-json-token",
            "username": "alice"
        }));
        let rendered = payload.to_string();
        for secret in [
            "lfm-json-key",
            "lfm-json-secret",
            "lfm-json-sig",
            "lfm-json-sk",
            "lb-json-token",
        ] {
            assert!(
                !rendered.contains(secret),
                "json_payload leaked `{secret}` in `{rendered}`"
            );
        }
        assert!(rendered.contains("alice"));
        // Normal words ending with `sk=` in prose must not wipe unrelated prefixes via FULL_LINE_MARKERS.
        assert!(!sensitive_text("disk=nvme0n1").contains(REDACTED_SECRET));
    }
}

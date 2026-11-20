use reqwest::Url;
use serde_json::{json, Value};

const REDACTED_STREAM: &str = "[redacted-stream-url]";
const REDACTED_SECRET: &str = "[redacted-sensitive-value]";

pub(crate) fn stream_url(input: &str) -> String {
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

pub(crate) fn sensitive_text(text: &str) -> String {
    text.lines()
        .take(12)
        .map(redact_line)
        .collect::<Vec<_>>()
        .join(" | ")
}

fn redact_line(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let secret_at = [
        "authorization:",
        "authorization=",
        "cookie:",
        "cookie=",
        "cookies=",
        "sapisid:",
        "sapisid=",
    ]
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
            output.push_str(&part[..position]);
            output.push_str(&stream_url(&part[position..]));
        } else {
            output.push_str(part);
        }
    }
    output
}

pub(crate) fn json_payload(value: &Value) -> Value {
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
                    } else if lower.contains("authorization")
                        || lower.contains("cookie")
                        || lower.contains("http-header")
                    {
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
}

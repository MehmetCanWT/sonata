//! Adversarial redaction and deterministic fuzz-corpus tests for high-risk `sonata-core` parsers.
//!
//! 1. **Parser Fuzzing**: Exercises cookie headers, cipher URL/query reconstruction, YouTube Music
//!    and JioSaavn payload parsers, Spotify playlist embed HTML, and LRC/TTML lyrics parsers across
//!    thousands of deterministic mutated inputs to prove panic-freedom.
//! 2. **Adversarial Redaction**: Verifies that raw cookies, `Authorization`, `SAPISID`, `sp_dc`,
//!    signed stream URLs, and nested JSON payloads never leak credentials after redaction.

use serde_json::{json, Value};
use sonata_core::auth::{parse_netscape_cookies, ExtractedCookies};
use sonata_core::lyrics::{parse_lrc, ttml::parse_ttml};
use sonata_core::providers::hifi::{apply_cipher_solution_to_url, SolvedCipherPair};
use sonata_core::providers::jiosaavn::{
    decrypt_url, parse_search_results_json, parse_song_details_json, select_best_stream_url,
};
use sonata_core::providers::spotify::parse_embed_playlist_html;
use sonata_core::providers::ytmusic::{
    parse_account_profile_json, parse_album_detail_json, parse_artist_banner_url_json,
    parse_artist_page_json, parse_cards_tree_json, parse_home_shelves_json, parse_mood_chips_json,
    parse_mood_tiles_json, parse_rich_search_json, parse_search_suggestions_json, HomeCardKind,
};
use sonata_core::redaction::{json_payload, sensitive_text, stream_url};

struct XorShift64(u64);

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn gen_bytes(&mut self, max_len: usize) -> Vec<u8> {
        let len = (self.next_u64() as usize) % (max_len + 1);
        (0..len).map(|_| (self.next_u64() & 0xff) as u8).collect()
    }

    fn gen_adversarial_str(&mut self, seeds: &[&str]) -> String {
        let base = seeds[(self.next_u64() as usize) % seeds.len()];
        let mut out = base.to_string();
        let mutations = (self.next_u64() % 5) as usize;
        const TOKENS: &[&str] = &[
            "\t",
            "\n",
            "\r\n",
            "\0",
            "\"",
            "'",
            "\\",
            "&",
            "=",
            "?",
            "#",
            "%ZZ",
            "%20",
            "https://",
            "music.youtube.com",
            "SAPISID=",
            "sp_dc=",
            "{",
            "}",
            "[",
            "]",
            "<p>",
            "</p>",
            "00:01.23",
            "TRUE",
            "FALSE",
            "999999999999999999999",
            "İıŞşĞğ",
        ];
        for _ in 0..mutations {
            let tok = TOKENS[(self.next_u64() as usize) % TOKENS.len()];
            let idx = (self.next_u64() as usize) % (out.len() + 1);
            let safe_idx = out
                .char_indices()
                .map(|(i, _)| i)
                .find(|&i| i >= idx)
                .unwrap_or(out.len());
            out.insert_str(safe_idx, tok);
        }
        if self.next_u64().is_multiple_of(4) {
            let raw = self.gen_bytes(64);
            out.push_str(&String::from_utf8_lossy(&raw));
        }
        out
    }
}

#[test]
fn fuzz_cookie_headers_and_netscape_parser_never_panics() {
    let mut rng = XorShift64::new(0xC00B_1E55_CAFE_BABEu64);
    let seeds = [
        ".youtube.com\tTRUE\t/\tTRUE\t1893456000\tSAPISID\tabc123",
        "# comment line\n.spotify.com\tTRUE\t/\tTRUE\t0\tsp_dc\ttoken_val",
        "SAPISID=abc; __Secure-3PAPISID=def; sp_dc=ghi; malformed",
        "",
    ];
    for _ in 0..600 {
        let sample = rng.gen_adversarial_str(&seeds);
        let _ = parse_netscape_cookies(&sample);
        let cookies = ExtractedCookies::from_ytm_cookie_header(&sample);
        let _ = cookies.ytm_sapisidhash();
    }
}

#[test]
fn fuzz_cipher_url_and_query_reconstruction_never_panics() {
    let mut rng = XorShift64::new(0xC1F3_4000_1234_5678u64);
    let seeds = [
        "https://rr1---sn.googlevideo.com/videoplayback?expire=1700000000&n=abc123&itag=251",
        "https://example.test/media?sp=sig&s=rawsig&n=old#frag",
        "not-even-a-url?a=1&b=2",
        "",
    ];
    for _ in 0..600 {
        let url = rng.gen_adversarial_str(&seeds);
        let cipher = rng.gen_adversarial_str(&[
            "s=rawsig&sp=sig&url=https%3A%2F%2Frr1.googlevideo.com%2Fvideoplayback%3Fn%3Dold",
            "s=a%26b&url=http%3A%2F%2Finsecure.test%2Fpath",
            "",
        ]);
        let sig = rng.gen_adversarial_str(&["deciphered_sig", "a=b&c=d", ""]);
        let n_val = rng.gen_adversarial_str(&["new_n", "n%20val", ""]);
        let _ = apply_cipher_solution_to_url(Some(&url), Some(&cipher), |_, _| {
            Some(SolvedCipherPair {
                sig: Some(sig.clone()),
                n: Some(n_val.clone()),
            })
        });
        let _ = apply_cipher_solution_to_url(None, Some(&cipher), |_, _| None);
    }
}

#[test]
fn fuzz_provider_payload_parsers_and_jiosaavn_decryptor_never_panic() {
    let mut rng = XorShift64::new(0x9A5E_D00D_F00D_1337u64);
    let json_seeds = [
        r#"{"contents":{"sectionListRenderer":{"contents":[{"musicShelfRenderer":{"title":{"runs":[{"text":"Songs"}]},"contents":[{}]}}]}},"header":{"musicResponsiveHeaderRenderer":{"title":{"runs":[{"text":"Album"}]}}}}"#,
        r#"{"results":[{"id":"1","title":"Song &amp; Title","more_info":{"duration":"200","encrypted_media_url":"AAAA"}}]}"#,
        r#"{"songs":[{"title":"Track","more_info":{"encrypted_media_url":"invalid","320kbps":"true"}}]}"#,
        r#"[]"#,
        r#"null"#,
        r#"{broken json"#,
    ];

    for _ in 0..500 {
        let raw = rng.gen_adversarial_str(&json_seeds);
        let _ = parse_search_results_json(&raw);
        let _ = parse_song_details_json("1", &raw);
        let _ = decrypt_url(&raw);
        let _ = select_best_stream_url(&raw, rng.next_u64().is_multiple_of(2));

        if let Ok(val) = serde_json::from_str::<Value>(&raw) {
            let _ = parse_home_shelves_json(&val);
            let _ = parse_account_profile_json(&val);
            let _ = parse_artist_page_json("UCtest", &val);
            let _ = parse_mood_chips_json(&val);
            let _ = parse_mood_tiles_json(&val);
            let _ = parse_cards_tree_json(&val, HomeCardKind::Album);
            let _ = parse_artist_banner_url_json(&val);
            let _ = parse_album_detail_json("MPREtest", &val);
            let _ = parse_rich_search_json(&val);
            let _ = parse_search_suggestions_json(&val);
        }
    }
}

#[test]
fn fuzz_playlist_embed_and_lyrics_parsers_never_panic() {
    let mut rng = XorShift64::new(0x1F1C_5A11_7777_8888u64);
    let html_seeds = [
        r#"<script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"state":{"data":{"entity":{"name":"Mix","trackList":[]}}}}}}</script>"#,
        r#"<html><body>unexpected</body></html>"#,
        "",
    ];
    let lyrics_seeds = [
        "[00:12.34] Hello <00:12.50> world (backing)\n[99:99.99] end",
        r#"<tt xmlns="http://www.w3.org/ns/ttml"><body><div><p begin="00:01.000" end="00:03.000"><span>word</span></p></div></body></tt>"#,
        "[[[[[:::::>>>>>",
        "",
    ];

    for _ in 0..500 {
        let html = rng.gen_adversarial_str(&html_seeds);
        let _ = parse_embed_playlist_html(&html);

        let lyric = rng.gen_adversarial_str(&lyrics_seeds);
        let _ = parse_lrc(&lyric);
        let _ = parse_ttml(&lyric);
    }
}

#[test]
fn redacts_adversarial_urls_headers_and_nested_json_secrets() {
    let cookie_secret = "SUPER_SECRET_SAPISID_VALUE_999";
    let sp_dc_secret = "AQB_SUPER_SECRET_SP_DC_TOKEN_888";
    let bearer_secret = "eyJhbGciOiJFUzI1NiJ9.SECRET_PAYLOAD.SECRET_SIG";
    let sig_secret = "VERY_SECRET_GOOG_SIGNATURE_777";

    let adversarial_text = format!(
        "Request failed:\n\
         Cookie: SAPISID={cookie_secret}; __Secure-1PAPISID={cookie_secret}; sp_dc={sp_dc_secret}\n\
         Authorization: SAPISIDHASH 1700000000_abcdef0123456789\n\
         Authorization: Bearer {bearer_secret}\n\
         URL: https://rr3---sn.googlevideo.com/videoplayback?expire=1900000000&sig={sig_secret}&n=secret_n_val&key=yt8"
    );
    let scrubbed = sensitive_text(&adversarial_text);
    assert!(
        !scrubbed.contains(cookie_secret),
        "SAPISID leaked: {scrubbed}"
    );
    assert!(!scrubbed.contains(sp_dc_secret), "sp_dc leaked: {scrubbed}");
    assert!(
        !scrubbed.contains(bearer_secret),
        "Bearer JWT leaked: {scrubbed}"
    );
    assert!(
        !scrubbed.contains("abcdef0123456789"),
        "SAPISIDHASH leaked: {scrubbed}"
    );
    assert!(
        !scrubbed.contains(sig_secret),
        "Signed stream sig leaked: {scrubbed}"
    );

    let signed_url = format!(
        "https://rr1---sn.googlevideo.com/videoplayback?itag=774&sig={sig_secret}&lsig=SECRET_LSIG&token=SECRET_TOK"
    );
    let scrubbed_url = stream_url(&signed_url);
    assert!(!scrubbed_url.contains(sig_secret));
    assert!(!scrubbed_url.contains("SECRET_LSIG"));
    assert!(!scrubbed_url.contains("SECRET_TOK"));

    let nested = json!({
        "command": [
            "loadfile",
            signed_url,
            "replace",
            -1,
            {
                "http-header-fields": format!("Cookie=SAPISID={cookie_secret},Authorization=Bearer {bearer_secret}"),
                "nested": {
                    "Cookie": format!("sp_dc={sp_dc_secret}"),
                    "Authorization": format!("SAPISIDHASH 123_{cookie_secret}")
                }
            }
        ]
    });
    let scrubbed_json = json_payload(&nested).to_string();
    assert!(!scrubbed_json.contains(cookie_secret));
    assert!(!scrubbed_json.contains(sp_dc_secret));
    assert!(!scrubbed_json.contains(bearer_secret));
    assert!(!scrubbed_json.contains(sig_secret));
}

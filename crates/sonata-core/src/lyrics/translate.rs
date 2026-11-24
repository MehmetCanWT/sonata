//! Portable lyrics batch translation and script romanization for Sonata.
//!
//! 1. **Batch Translation & Remote Romanization**: `translate_lyrics_lines` and
//!    `romanize_lyrics_lines` preserve 1:1 `LyricsState` line alignment across bounded
//!    concurrent `reqwest` batches using Private Use Area (`\u{E000}...\u{E001}`) delimiters.
//! 2. **Portable Script Romanization**: `romanize_text` and `romanize_lines_offline`
//!    provide zero-subprocess transliteration for Japanese (Hiragana/Katakana with
//!    Kanji awareness), Korean (Hangul Revised Romanization), Cyrillic, and Greek.
//! 3. **Private On-Disk Caching**: Stores line-aligned JSON cache entries under
//!    `CorePaths::lyrics_cache_dir()` using `0o700` directories and atomic `0o600` files
//!    keyed by track ID, mode, and target language.

use crate::auth::{create_private_file, ensure_private_dir};
use crate::error::CoreError;
use crate::lyrics::LyricsState;
use crate::paths::CorePaths;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

/// Default Google Translate lightweight endpoint matching Android's `LyricsTranslator`.
pub const GOOGLE_TRANSLATE_ENDPOINT: &str = "https://translate.googleapis.com/translate_a/single";

/// Maximum character budget per batched request payload.
pub const MAX_BATCH_CHARS: usize = 3_200;

/// Maximum number of concurrent batch HTTP requests in flight.
pub const MAX_CONCURRENT_BATCHES: usize = 2;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const TRANSLATE_USER_AGENT: &str = "Sonata/1.0";
const CACHE_SCHEMA_VERSION: u32 = 1;
static TEMP_FILE_SEQ: AtomicU64 = AtomicU64::new(0);

/// High-level script classification discovered in a lyric line or track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptKind {
    Latin,
    JapaneseKana,
    Han,
    Hangul,
    Cyrillic,
    Greek,
    OtherNonLatin,
}

/// Detailed output from portable offline romanization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomanizedText {
    /// Romanized text (with any untransliterable Han/Kanji characters preserved as-is).
    pub text: String,
    /// Whether at least one character was transliterated into Latin script.
    pub changed: bool,
    /// Whether the input contained Han/Kanji ideographs that cannot be deterministically
    /// romanized offline without lexical context.
    pub has_untransliterated_han: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Slot {
    original_index: usize,
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Batch {
    slots: Vec<Slot>,
    payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CachedLyricsTransform {
    version: u32,
    track_id: String,
    mode: String,
    target_lang: String,
    source_hash: String,
    line_count: usize,
    lines: Vec<String>,
}

/// Extracts 1:1 lyric line strings from a `LyricsState` (`Synced` or `Plain`).
///
/// Returns `None` for `Idle`, `Loading`, `NotFound`, or empty lyrics slices.
pub fn extract_lyrics_lines(state: &LyricsState) -> Option<Vec<String>> {
    match state {
        LyricsState::Synced(lines) if !lines.is_empty() => {
            Some(lines.iter().map(|line| line.text.clone()).collect())
        }
        LyricsState::Plain(lines) if !lines.is_empty() => Some(lines.clone()),
        _ => None,
    }
}

/// Returns `true` when any line contains an alphabetic letter outside the Latin script.
pub fn contains_non_latin_letter(lines: &[String]) -> bool {
    lines
        .iter()
        .any(|line| text_contains_non_latin_letter(line))
}

/// Returns `true` when `text` contains at least one alphabetic letter outside the Latin script.
pub fn text_contains_non_latin_letter(text: &str) -> bool {
    text.chars().any(is_non_latin_letter)
}

/// Returns `true` when `text` contains any Han/Kanji/Hanja ideograph.
pub fn contains_han(text: &str) -> bool {
    text.chars().any(is_han_char)
}

/// Returns the primary non-Latin script encountered in `text`, or `ScriptKind::Latin` when none.
pub fn detect_primary_script(text: &str) -> ScriptKind {
    for ch in text.chars() {
        if is_hiragana(ch) || is_katakana(ch) || is_halfwidth_katakana(ch) {
            return ScriptKind::JapaneseKana;
        }
        if is_hangul_syllable(ch) || is_hangul_jamo(ch) {
            return ScriptKind::Hangul;
        }
        if is_cyrillic(ch) {
            return ScriptKind::Cyrillic;
        }
        if is_greek(ch) {
            return ScriptKind::Greek;
        }
        if is_han_char(ch) {
            return ScriptKind::Han;
        }
        if is_non_latin_letter(ch) {
            return ScriptKind::OtherNonLatin;
        }
    }
    ScriptKind::Latin
}

/// Translates `lines` into `target_lang` while preserving blank lines and exact 1:1 indices.
///
/// Checks the private JSON cache under `paths.lyrics_cache_dir()` before issuing bounded
/// batched requests via `http`. Returns `Ok(None)` for empty `lines` or blank `target_lang`.
pub async fn translate_lyrics_lines(
    http: &Client,
    paths: &CorePaths,
    track_id: &str,
    lines: &[String],
    target_lang: &str,
) -> Result<Option<Vec<String>>, CoreError> {
    translate_lyrics_lines_with_endpoint(
        http,
        paths,
        track_id,
        lines,
        target_lang,
        GOOGLE_TRANSLATE_ENDPOINT,
    )
    .await
}

/// Translates `LyricsState` (`Synced` or `Plain`) lines into `target_lang` with 1:1 alignment.
pub async fn translate_lyrics_state(
    http: &Client,
    paths: &CorePaths,
    track_id: &str,
    state: &LyricsState,
    target_lang: &str,
) -> Result<Option<Vec<String>>, CoreError> {
    let Some(lines) = extract_lyrics_lines(state) else {
        return Ok(None);
    };
    translate_lyrics_lines(http, paths, track_id, &lines, target_lang).await
}

/// Romanizes non-Latin `lines` into Latin script while preserving blank lines and 1:1 indices.
///
/// Skips lines/tracks that contain only Latin letters and punctuation (`Ok(None)`). Checks the
/// private JSON cache under `paths.lyrics_cache_dir()`, queries the remote romanization endpoint,
/// and falls back to deterministic portable script romanization when no Han/Kanji ideographs
/// require dictionary lookup.
pub async fn romanize_lyrics_lines(
    http: &Client,
    paths: &CorePaths,
    track_id: &str,
    lines: &[String],
) -> Result<Option<Vec<String>>, CoreError> {
    romanize_lyrics_lines_with_endpoint(http, paths, track_id, lines, GOOGLE_TRANSLATE_ENDPOINT)
        .await
}

/// Romanizes `LyricsState` (`Synced` or `Plain`) lines into Latin script with 1:1 alignment.
pub async fn romanize_lyrics_state(
    http: &Client,
    paths: &CorePaths,
    track_id: &str,
    state: &LyricsState,
) -> Result<Option<Vec<String>>, CoreError> {
    let Some(lines) = extract_lyrics_lines(state) else {
        return Ok(None);
    };
    romanize_lyrics_lines(http, paths, track_id, &lines).await
}

/// Endpoint-injectable translation helper used by `translate_lyrics_lines` and tests.
pub async fn translate_lyrics_lines_with_endpoint(
    http: &Client,
    paths: &CorePaths,
    track_id: &str,
    lines: &[String],
    target_lang: &str,
    endpoint: &str,
) -> Result<Option<Vec<String>>, CoreError> {
    let target = target_lang.trim();
    if target.is_empty() || lines.is_empty() {
        return Ok(None);
    }

    let normalized_target = target.to_ascii_lowercase();
    if let Some(cached) =
        load_cached_lines_async(paths, track_id, "translate", &normalized_target, lines).await
    {
        return Ok(Some(cached));
    }

    let slots = collect_slots(lines);
    if slots.is_empty() {
        return Ok(Some(lines.to_vec()));
    }

    let batches = build_batches(&slots);
    let translated_slots = execute_batches(http, endpoint, &batches, target, false).await?;
    if translated_slots.len() != slots.len() {
        return Err(CoreError::invalid_response(
            "translate",
            format!(
                "translated slot count mismatch: expected {}, got {}",
                slots.len(),
                translated_slots.len()
            ),
        ));
    }

    let merged = merge_back(lines, &slots, translated_slots);
    save_cached_lines_async(
        paths,
        track_id,
        "translate",
        &normalized_target,
        lines,
        &merged,
    )
    .await;
    Ok(Some(merged))
}

/// Endpoint-injectable romanization helper used by `romanize_lyrics_lines` and tests.
pub async fn romanize_lyrics_lines_with_endpoint(
    http: &Client,
    paths: &CorePaths,
    track_id: &str,
    lines: &[String],
    endpoint: &str,
) -> Result<Option<Vec<String>>, CoreError> {
    if lines.is_empty() {
        return Ok(None);
    }

    let slots = collect_slots(lines);
    if slots.is_empty() {
        return Ok(Some(lines.to_vec()));
    }
    if !contains_non_latin_letter(lines) {
        return Ok(None);
    }

    if let Some(cached) = load_cached_lines_async(paths, track_id, "romanize", "rm", lines).await {
        return Ok(Some(cached));
    }

    let batches = build_batches(&slots);
    match execute_batches(http, endpoint, &batches, "en", true).await {
        Ok(romanized_slots) if romanized_slots.len() == slots.len() => {
            let merged = merge_back(lines, &slots, romanized_slots);
            save_cached_lines_async(paths, track_id, "romanize", "rm", lines, &merged).await;
            Ok(Some(merged))
        }
        Ok(romanized_slots) => {
            let err = CoreError::invalid_response(
                "translate",
                format!(
                    "romanized slot count mismatch: expected {}, got {}",
                    slots.len(),
                    romanized_slots.len()
                ),
            );
            romanize_offline_fallback(paths, track_id, lines, err).await
        }
        Err(err) => romanize_offline_fallback(paths, track_id, lines, err).await,
    }
}

async fn romanize_offline_fallback(
    paths: &CorePaths,
    track_id: &str,
    lines: &[String],
    remote_err: CoreError,
) -> Result<Option<Vec<String>>, CoreError> {
    // Only substitute deterministic offline romanization when no Han/Kanji ideographs exist,
    // because kanji readings depend on lexical context and would leave partial raw characters.
    let has_han = lines.iter().any(|line| contains_han(line));
    if !has_han {
        if let Some(offline) = romanize_lines_offline(lines) {
            save_cached_lines_async(paths, track_id, "romanize", "rm", lines, &offline).await;
            return Ok(Some(offline));
        }
    }
    Err(remote_err)
}

/// Deterministically romanizes `text` offline across Japanese Kana, Korean Hangul (RR),
/// Cyrillic, and Greek, leaving Latin text and untransliterable Han/Kanji intact.
pub fn romanize_text(text: &str) -> String {
    romanize_text_with_report(text).text
}

/// Deterministically romanizes `text` offline and reports whether characters changed or Han
/// ideographs remained untransliterated.
pub fn romanize_text_with_report(text: &str) -> RomanizedText {
    if text.is_empty() {
        return RomanizedText {
            text: String::new(),
            changed: false,
            has_untransliterated_han: false,
        };
    }

    let normalized = normalize_fullwidth_and_halfwidth_kana(text);
    let chars: Vec<char> = normalized.chars().collect();
    let mut out = String::with_capacity(normalized.len() * 2);
    let mut changed = normalized != text;
    let mut has_han = false;
    let mut i = 0usize;

    while i < chars.len() {
        let ch = chars[i];

        if is_han_char(ch) {
            has_han = true;
            out.push(ch);
            i += 1;
            continue;
        }

        if is_kana(ch) {
            let consumed = transliterate_kana_at(&chars, i, &mut out);
            changed = true;
            i += consumed;
            continue;
        }

        if ch == 'ー' {
            if let Some(vowel) = last_ascii_vowel(&out) {
                out.push(vowel);
            } else {
                out.push('-');
            }
            changed = true;
            i += 1;
            continue;
        }

        if is_hangul_syllable(ch) {
            let consumed = transliterate_hangul_word(&chars, i, &mut out);
            changed = true;
            i += consumed;
            continue;
        }

        if let Some(jamo) = transliterate_compat_jamo(ch) {
            out.push_str(jamo);
            changed = true;
            i += 1;
            continue;
        }

        if is_cyrillic(ch) {
            let prev = i.checked_sub(1).map(|idx| chars[idx]);
            let next = chars.get(i + 1).copied();
            transliterate_cyrillic_char(ch, prev, next, &mut out);
            changed = true;
            i += 1;
            continue;
        }

        if is_greek(ch) {
            let prev = i.checked_sub(1).map(|idx| chars[idx]);
            let next = chars.get(i + 1).copied();
            let consumed = transliterate_greek_at(&chars, i, prev, next, &mut out);
            changed = true;
            i += consumed;
            continue;
        }

        out.push(ch);
        i += 1;
    }

    RomanizedText {
        text: out,
        changed,
        has_untransliterated_han: has_han,
    }
}

/// Offline line-by-line romanizer preserving blank lines and 1:1 alignment.
///
/// Returns `None` when `lines` is empty, contains no non-Latin letters, or no line changed
/// (for example, pure Han/Kanji text where no deterministic script mapping applies).
pub fn romanize_lines_offline(lines: &[String]) -> Option<Vec<String>> {
    if lines.is_empty() {
        return None;
    }
    let slots = collect_slots(lines);
    if slots.is_empty() {
        return Some(lines.to_vec());
    }
    if !contains_non_latin_letter(lines) {
        return None;
    }

    let mut transformed = Vec::with_capacity(slots.len());
    let mut any_changed = false;
    for slot in &slots {
        let res = romanize_text_with_report(&slot.text);
        if res.changed {
            any_changed = true;
        }
        transformed.push(res.text);
    }

    if !any_changed {
        return None;
    }
    Some(merge_back(lines, &slots, transformed))
}

fn collect_slots(lines: &[String]) -> Vec<Slot> {
    lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(Slot {
                    original_index: idx,
                    text: trimmed.to_string(),
                })
            }
        })
        .collect()
}

fn marker_for(index: usize) -> String {
    format!("\u{E000}{index:04}\u{E001}")
}

fn build_batches(slots: &[Slot]) -> Vec<Batch> {
    let mut result = Vec::new();
    let mut current: Vec<Slot> = Vec::new();
    let mut current_len = 0usize;

    let flush = |result: &mut Vec<Batch>, current: &mut Vec<Slot>, current_len: &mut usize| {
        if current.is_empty() {
            return;
        }
        let mut payload = String::new();
        for (idx, slot) in current.iter().enumerate() {
            if idx > 0 {
                payload.push('\n');
                payload.push_str(&marker_for(idx));
                payload.push('\n');
            }
            payload.push_str(&slot.text);
        }
        result.push(Batch {
            slots: std::mem::take(current),
            payload,
        });
        *current_len = 0;
    };

    for slot in slots {
        // Match Android's character-budget accounting: 8 chars for "\n\uE0000000\uE001\n".
        let slot_chars = slot.text.chars().count();
        let delta = slot_chars + if current.is_empty() { 0 } else { 8 };
        if !current.is_empty() && current_len + delta > MAX_BATCH_CHARS {
            flush(&mut result, &mut current, &mut current_len);
        }
        current.push(slot.clone());
        current_len += delta;
    }
    flush(&mut result, &mut current, &mut current_len);
    result
}

async fn execute_batches(
    http: &Client,
    endpoint: &str,
    batches: &[Batch],
    target_lang: &str,
    romanize: bool,
) -> Result<Vec<String>, CoreError> {
    let mut all = Vec::new();
    for chunk in batches.chunks(MAX_CONCURRENT_BATCHES) {
        match chunk {
            [single] => {
                let parts = request_batch(http, endpoint, single, target_lang, romanize).await?;
                all.extend(parts);
            }
            [first, second] => {
                let (res_a, res_b) = tokio::join!(
                    request_batch(http, endpoint, first, target_lang, romanize),
                    request_batch(http, endpoint, second, target_lang, romanize),
                );
                all.extend(res_a?);
                all.extend(res_b?);
            }
            _ => {
                for batch in chunk {
                    let parts = request_batch(http, endpoint, batch, target_lang, romanize).await?;
                    all.extend(parts);
                }
            }
        }
    }
    Ok(all)
}

async fn request_batch(
    http: &Client,
    endpoint: &str,
    batch: &Batch,
    target_lang: &str,
    romanize: bool,
) -> Result<Vec<String>, CoreError> {
    let dt = if romanize { "rm" } else { "t" };
    let form_body = format!(
        "client=dict-chrome-ex&sl=auto&tl={}&dt={dt}&q={}",
        urlencoding::encode(target_lang),
        urlencoding::encode(&batch.payload)
    );

    let resp = http
        .post(endpoint)
        .header("User-Agent", TRANSLATE_USER_AGENT)
        .header("Accept", "application/json")
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        )
        .timeout(REQUEST_TIMEOUT)
        .body(form_body)
        .send()
        .await
        .map_err(|err| CoreError::from_reqwest("translate", &err))?;

    let status = resp.status();
    if status.as_u16() == 429 {
        let retry_after = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|raw| raw.trim().parse::<u64>().ok())
            .map(Duration::from_secs);
        return Err(CoreError::rate_limited(
            retry_after,
            "Translate endpoint returned HTTP 429 Too Many Requests",
        ));
    }
    if !status.is_success() {
        return Err(CoreError::invalid_response(
            "translate",
            format!("Translate endpoint returned HTTP {}", status.as_u16()),
        ));
    }

    let body = resp
        .text()
        .await
        .map_err(|err| CoreError::from_reqwest("translate", &err))?;

    parse_translate_response(&body, batch.slots.len(), romanize)
}

fn parse_translate_response(
    body: &str,
    expected_slots: usize,
    romanize: bool,
) -> Result<Vec<String>, CoreError> {
    let root: Value = serde_json::from_str(body).map_err(|err| {
        CoreError::invalid_response("translate", format!("malformed JSON: {err}"))
    })?;
    let segments = root
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            CoreError::invalid_response("translate", "missing top-level segments array")
        })?;

    let field_idx = if romanize { 3 } else { 0 };
    let mut combined = String::new();
    for seg in segments {
        let Some(piece) = seg
            .as_array()
            .and_then(|arr| arr.get(field_idx))
            .and_then(|v| v.as_str())
        else {
            continue;
        };
        if !piece.is_empty() && piece != "null" {
            combined.push_str(piece);
        }
    }

    if combined.trim().is_empty() {
        return Err(CoreError::invalid_response(
            "translate",
            "translate response produced empty text",
        ));
    }

    static MARKER_SPLIT_RE: OnceLock<Regex> = OnceLock::new();
    let re = MARKER_SPLIT_RE
        .get_or_init(|| Regex::new(r"\u{E000}[^\u{E000}\u{E001}]*[\u{E000}\u{E001}]").unwrap());

    let parts: Vec<String> = re
        .split(&combined)
        .map(|part| part.trim().to_string())
        .collect();

    if parts.len() != expected_slots || parts.iter().any(|part| part.trim().is_empty()) {
        return Err(CoreError::invalid_response(
            "translate",
            format!(
                "marker split produced {} parts for {} expected lines",
                parts.len(),
                expected_slots
            ),
        ));
    }

    Ok(parts)
}

fn merge_back(original: &[String], slots: &[Slot], transformed: Vec<String>) -> Vec<String> {
    let mut out = original.to_vec();
    for (slot, text) in slots.iter().zip(transformed) {
        if let Some(cell) = out.get_mut(slot.original_index) {
            *cell = text;
        }
    }
    out
}

fn sha1_hex(input: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(input);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn compute_lines_hash(lines: &[String]) -> String {
    let joined = lines.join("\n");
    sha1_hex(joined.as_bytes())
}

/// Computes the deterministic JSON cache file path under `paths.lyrics_cache_dir()`
/// for `track_id`, `mode`, and `target_lang`.
pub fn translate_cache_path(
    paths: &CorePaths,
    track_id: &str,
    mode: &str,
    target_lang: &str,
) -> PathBuf {
    let key = format!(
        "{mode}|{}|{}",
        track_id.trim(),
        target_lang.trim().to_ascii_lowercase()
    );
    let hex = sha1_hex(key.as_bytes());
    paths.lyrics_cache_dir().join(format!("tr_{hex}.json"))
}

/// Synchronously reads cached transformed lyrics from `paths.lyrics_cache_dir()` if the entry
/// matches `track_id`, `mode`, `target_lang`, and `source_lines`.
pub fn load_cached_lyrics_lines(
    paths: &CorePaths,
    track_id: &str,
    mode: &str,
    target_lang: &str,
    source_lines: &[String],
) -> Option<Vec<String>> {
    let trimmed_id = track_id.trim();
    if trimmed_id.is_empty() || source_lines.is_empty() {
        return None;
    }
    let path = translate_cache_path(paths, trimmed_id, mode, target_lang);
    let raw = fs::read_to_string(&path).ok()?;
    let entry: CachedLyricsTransform = serde_json::from_str(&raw).ok()?;
    let expected_lang = target_lang.trim().to_ascii_lowercase();
    let expected_hash = compute_lines_hash(source_lines);
    if entry.version == CACHE_SCHEMA_VERSION
        && entry.track_id == trimmed_id
        && entry.mode == mode
        && entry.target_lang == expected_lang
        && entry.line_count == source_lines.len()
        && entry.lines.len() == source_lines.len()
        && entry.source_hash == expected_hash
    {
        Some(entry.lines)
    } else {
        None
    }
}

/// Synchronously persists transformed lyrics into `paths.lyrics_cache_dir()` with `0o700`
/// directory and atomic `0o600` file permissions on Unix.
pub fn save_cached_lyrics_lines(
    paths: &CorePaths,
    track_id: &str,
    mode: &str,
    target_lang: &str,
    source_lines: &[String],
    transformed_lines: &[String],
) -> Result<(), CoreError> {
    let trimmed_id = track_id.trim();
    if trimmed_id.is_empty() || source_lines.len() != transformed_lines.len() {
        return Ok(());
    }
    let path = translate_cache_path(paths, trimmed_id, mode, target_lang);
    let parent = path.parent().ok_or_else(|| {
        CoreError::io(format!(
            "lyrics translate cache path {} has no parent directory",
            path.display()
        ))
    })?;
    ensure_private_dir(parent)
        .map_err(|err| CoreError::io(format!("failed to create {}: {err}", parent.display())))?;

    let payload = CachedLyricsTransform {
        version: CACHE_SCHEMA_VERSION,
        track_id: trimmed_id.to_string(),
        mode: mode.to_string(),
        target_lang: target_lang.trim().to_ascii_lowercase(),
        source_hash: compute_lines_hash(source_lines),
        line_count: source_lines.len(),
        lines: transformed_lines.to_vec(),
    };
    let json = serde_json::to_string(&payload)
        .map_err(|err| CoreError::cache(format!("failed to serialize lyrics cache: {err}")))?;

    write_private_atomic(parent, &path, json.as_bytes())
}

fn write_private_atomic(parent: &Path, path: &Path, bytes: &[u8]) -> Result<(), CoreError> {
    let seq = TEMP_FILE_SEQ.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("tr_cache.json");
    let tmp_path = parent.join(format!(".{file_name}.{}.{seq}.tmp", std::process::id()));

    let write_res = (|| -> std::io::Result<()> {
        let mut file = create_private_file(&tmp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp_path, path)?;
        Ok(())
    })();

    if let Err(err) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(CoreError::io(format!(
            "failed to write lyrics cache {}: {err}",
            path.display()
        )));
    }
    Ok(())
}

async fn load_cached_lines_async(
    paths: &CorePaths,
    track_id: &str,
    mode: &str,
    target_lang: &str,
    source_lines: &[String],
) -> Option<Vec<String>> {
    if track_id.trim().is_empty() || source_lines.is_empty() {
        return None;
    }
    let paths = paths.clone();
    let track_id = track_id.to_string();
    let mode = mode.to_string();
    let target_lang = target_lang.to_string();
    let source_lines = source_lines.to_vec();
    tokio::task::spawn_blocking(move || {
        load_cached_lyrics_lines(&paths, &track_id, &mode, &target_lang, &source_lines)
    })
    .await
    .ok()
    .flatten()
}

async fn save_cached_lines_async(
    paths: &CorePaths,
    track_id: &str,
    mode: &str,
    target_lang: &str,
    source_lines: &[String],
    transformed_lines: &[String],
) {
    if track_id.trim().is_empty() || source_lines.len() != transformed_lines.len() {
        return;
    }
    let paths = paths.clone();
    let track_id = track_id.to_string();
    let mode = mode.to_string();
    let target_lang = target_lang.to_string();
    let source_lines = source_lines.to_vec();
    let transformed_lines = transformed_lines.to_vec();
    if let Err(err) = tokio::task::spawn_blocking(move || {
        save_cached_lyrics_lines(
            &paths,
            &track_id,
            &mode,
            &target_lang,
            &source_lines,
            &transformed_lines,
        )
    })
    .await
    {
        tracing::warn!("[sonata:lyrics] translate cache join error: {err}");
    }
}

// ---------------------------------------------------------------------------
// Script Classification Helpers
// ---------------------------------------------------------------------------

fn is_non_latin_letter(ch: char) -> bool {
    ch.is_alphabetic() && !is_latin_alphabetic(ch)
}

fn is_latin_alphabetic(ch: char) -> bool {
    matches!(
        ch,
        'A'..='Z'
            | 'a'..='z'
            | '\u{00C0}'..='\u{00D6}'
            | '\u{00D8}'..='\u{00F6}'
            | '\u{00F8}'..='\u{024F}'
            | '\u{0250}'..='\u{02AF}'
            | '\u{1D00}'..='\u{1DBF}'
            | '\u{1E00}'..='\u{1EFF}'
            | '\u{2C60}'..='\u{2C7F}'
            | '\u{A720}'..='\u{A7FF}'
            | '\u{AB30}'..='\u{AB6F}'
            | '\u{FF21}'..='\u{FF3A}'
            | '\u{FF41}'..='\u{FF5A}'
    )
}

fn is_han_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{20000}'..='\u{2A6DF}'
            | '\u{2A700}'..='\u{2B73F}'
            | '\u{2B740}'..='\u{2B81F}'
            | '\u{2B820}'..='\u{2CEAF}'
            | '\u{2F800}'..='\u{2FA1F}'
    )
}

fn is_hiragana(ch: char) -> bool {
    ('\u{3040}'..='\u{309F}').contains(&ch)
}

fn is_katakana(ch: char) -> bool {
    ('\u{30A0}'..='\u{30FF}').contains(&ch) && ch != 'ー'
}

fn is_halfwidth_katakana(ch: char) -> bool {
    ('\u{FF65}'..='\u{FF9F}').contains(&ch)
}

fn is_kana(ch: char) -> bool {
    is_hiragana(ch) || is_katakana(ch)
}

fn is_hangul_syllable(ch: char) -> bool {
    ('\u{AC00}'..='\u{D7A3}').contains(&ch)
}

fn is_hangul_jamo(ch: char) -> bool {
    ('\u{1100}'..='\u{11FF}').contains(&ch) || ('\u{3130}'..='\u{318F}').contains(&ch)
}

fn is_cyrillic(ch: char) -> bool {
    ('\u{0400}'..='\u{04FF}').contains(&ch)
}

fn is_greek(ch: char) -> bool {
    ('\u{0370}'..='\u{03FF}').contains(&ch) || ('\u{1F00}'..='\u{1FFF}').contains(&ch)
}

fn normalize_fullwidth_and_halfwidth_kana(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\u{3000}' {
            out.push(' ');
            i += 1;
            continue;
        }
        if ('\u{FF01}'..='\u{FF5E}').contains(&ch) {
            if let Some(ascii) = char::from_u32(ch as u32 - 0xFEE0) {
                out.push(ascii);
                i += 1;
                continue;
            }
        }
        if let Some(base) = halfwidth_katakana_base(ch) {
            let next = chars.get(i + 1).copied();
            if next == Some('\u{FF9E}') {
                if let Some(voiced) = apply_katakana_dakuten(base) {
                    out.push(voiced);
                    i += 2;
                    continue;
                }
            } else if next == Some('\u{FF9F}') {
                if let Some(semi) = apply_katakana_handakuten(base) {
                    out.push(semi);
                    i += 2;
                    continue;
                }
            }
            out.push(base);
            i += 1;
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn halfwidth_katakana_base(ch: char) -> Option<char> {
    Some(match ch {
        '｡' => '。',
        '｢' => '「',
        '｣' => '」',
        '､' => '、',
        '･' => '・',
        'ｦ' => 'ヲ',
        'ｧ' => 'ァ',
        'ｨ' => 'ィ',
        'ｩ' => 'ゥ',
        'ｪ' => 'ェ',
        'ｫ' => 'ォ',
        'ｬ' => 'ャ',
        'ｭ' => 'ュ',
        'ｮ' => 'ョ',
        'ｯ' => 'ッ',
        'ｰ' => 'ー',
        'ｱ' => 'ア',
        'ｲ' => 'イ',
        'ｳ' => 'ウ',
        'ｴ' => 'エ',
        'ｵ' => 'オ',
        'ｶ' => 'カ',
        'ｷ' => 'キ',
        'ｸ' => 'ク',
        'ｹ' => 'ケ',
        'ｺ' => 'コ',
        'ｻ' => 'サ',
        'ｼ' => 'シ',
        'ｽ' => 'ス',
        'ｾ' => 'セ',
        'ｿ' => 'ソ',
        'ﾀ' => 'タ',
        'ﾁ' => 'チ',
        'ﾂ' => 'ツ',
        'ﾃ' => 'テ',
        'ﾄ' => 'ト',
        'ﾅ' => 'ナ',
        'ﾆ' => 'ニ',
        'ﾇ' => 'ヌ',
        'ﾈ' => 'ネ',
        'ﾉ' => 'ノ',
        'ﾊ' => 'ハ',
        'ﾋ' => 'ヒ',
        'ﾌ' => 'フ',
        'ﾍ' => 'ヘ',
        'ﾎ' => 'ホ',
        'ﾏ' => 'マ',
        'ﾐ' => 'ミ',
        'ﾑ' => 'ム',
        'ﾒ' => 'メ',
        'ﾓ' => 'モ',
        'ﾔ' => 'ヤ',
        'ﾕ' => 'ユ',
        'ﾖ' => 'ヨ',
        'ﾗ' => 'ラ',
        'ﾘ' => 'リ',
        'ﾙ' => 'ル',
        'ﾚ' => 'レ',
        'ﾛ' => 'ロ',
        'ﾜ' => 'ワ',
        'ﾝ' => 'ン',
        _ => return None,
    })
}

fn apply_katakana_dakuten(ch: char) -> Option<char> {
    Some(match ch {
        'ウ' => 'ヴ',
        'カ' => 'ガ',
        'キ' => 'ギ',
        'ク' => 'グ',
        'ケ' => 'ゲ',
        'コ' => 'ゴ',
        'サ' => 'ザ',
        'シ' => 'ジ',
        'ス' => 'ズ',
        'セ' => 'ゼ',
        'ソ' => 'ゾ',
        'タ' => 'ダ',
        'チ' => 'ヂ',
        'ツ' => 'ヅ',
        'テ' => 'デ',
        'ト' => 'ド',
        'ハ' => 'バ',
        'ヒ' => 'ビ',
        'フ' => 'ブ',
        'ヘ' => 'ベ',
        'ホ' => 'ボ',
        _ => return None,
    })
}

fn apply_katakana_handakuten(ch: char) -> Option<char> {
    Some(match ch {
        'ハ' => 'パ',
        'ヒ' => 'ピ',
        'フ' => 'プ',
        'ヘ' => 'ペ',
        'ホ' => 'ポ',
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Japanese Kana -> Hepburn Romanization
// ---------------------------------------------------------------------------

fn to_hiragana(ch: char) -> char {
    if ('\u{30A1}'..='\u{30F6}').contains(&ch) {
        char::from_u32(ch as u32 - 0x60).unwrap_or(ch)
    } else {
        ch
    }
}

fn last_ascii_vowel(s: &str) -> Option<char> {
    s.chars()
        .rev()
        .find(|c| c.is_ascii_alphabetic())
        .filter(|c| matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u'))
}

fn peek_kana_roman(chars: &[char], idx: usize) -> Option<&'static str> {
    let ch = *chars.get(idx)?;
    let h1 = to_hiragana(ch);
    if let Some(&next) = chars.get(idx + 1) {
        let h2 = to_hiragana(next);
        if let Some(di) = kana_digraph(h1, h2) {
            return Some(di);
        }
    }
    kana_single(h1)
}

fn transliterate_kana_at(chars: &[char], idx: usize, out: &mut String) -> usize {
    let h1 = to_hiragana(chars[idx]);

    // Sokuon (っ / ッ) doubles the next consonant (or prepends 't' before "ch").
    if h1 == 'っ' {
        if let Some(next_rom) = peek_kana_roman(chars, idx + 1) {
            if next_rom.starts_with("ch") {
                out.push('t');
            } else if let Some(first) = next_rom.chars().next() {
                if first.is_ascii_alphabetic() && !matches!(first, 'a' | 'e' | 'i' | 'o' | 'u') {
                    out.push(first);
                }
            }
        }
        return 1;
    }

    // Syllabic N (ん / ン) adds an apostrophe before vowels or 'y' to avoid ambiguity.
    if h1 == 'ん' {
        out.push('n');
        if let Some(next_rom) = peek_kana_roman(chars, idx + 1) {
            if next_rom.starts_with(['a', 'e', 'i', 'o', 'u', 'y']) {
                out.push('\'');
            }
        }
        return 1;
    }

    if let Some(&next) = chars.get(idx + 1) {
        let h2 = to_hiragana(next);
        if let Some(di) = kana_digraph(h1, h2) {
            out.push_str(di);
            return 2;
        }
    }

    if let Some(single) = kana_single(h1) {
        out.push_str(single);
    } else {
        out.push(chars[idx]);
    }
    1
}

fn kana_digraph(h1: char, h2: char) -> Option<&'static str> {
    Some(match (h1, h2) {
        ('き', 'ゃ') => "kya",
        ('き', 'ゅ') => "kyu",
        ('き', 'ょ') => "kyo",
        ('し', 'ゃ') => "sha",
        ('し', 'ゅ') => "shu",
        ('し', 'ょ') => "sho",
        ('ち', 'ゃ') => "cha",
        ('ち', 'ゅ') => "chu",
        ('ち', 'ょ') => "cho",
        ('に', 'ゃ') => "nya",
        ('に', 'ゅ') => "nyu",
        ('に', 'ょ') => "nyo",
        ('ひ', 'ゃ') => "hya",
        ('ひ', 'ゅ') => "hyu",
        ('ひ', 'ょ') => "hyo",
        ('み', 'ゃ') => "mya",
        ('み', 'ゅ') => "myu",
        ('み', 'ょ') => "myo",
        ('り', 'ゃ') => "rya",
        ('り', 'ゅ') => "ryu",
        ('り', 'ょ') => "ryo",
        ('ぎ', 'ゃ') => "gya",
        ('ぎ', 'ゅ') => "gyu",
        ('ぎ', 'ょ') => "gyo",
        ('じ', 'ゃ') | ('ぢ', 'ゃ') => "ja",
        ('じ', 'ゅ') | ('ぢ', 'ゅ') => "ju",
        ('じ', 'ょ') | ('ぢ', 'ょ') => "jo",
        ('び', 'ゃ') => "bya",
        ('び', 'ゅ') => "byu",
        ('び', 'ょ') => "byo",
        ('ぴ', 'ゃ') => "pya",
        ('ぴ', 'ゅ') => "pyu",
        ('ぴ', 'ょ') => "pyo",
        // Loanword & extended katakana/hiragana digraphs
        ('ふ', 'ぁ') => "fa",
        ('ふ', 'ぃ') => "fi",
        ('ふ', 'ぇ') => "fe",
        ('ふ', 'ぉ') => "fo",
        ('て', 'ぃ') => "ti",
        ('で', 'ぃ') => "di",
        ('と', 'ぅ') => "tu",
        ('ど', 'ぅ') => "du",
        ('て', 'ゅ') => "tyu",
        ('で', 'ゅ') => "dyu",
        ('う', 'ぃ') => "wi",
        ('う', 'ぇ') => "we",
        ('う', 'ぉ') => "wo",
        ('ゔ', 'ぁ') => "va",
        ('ゔ', 'ぃ') => "vi",
        ('ゔ', 'ぇ') => "ve",
        ('ゔ', 'ぉ') => "vo",
        ('し', 'ぇ') => "she",
        ('じ', 'ぇ') => "je",
        ('ち', 'ぇ') => "che",
        ('つ', 'ぁ') => "tsa",
        ('つ', 'ぃ') => "tsi",
        ('つ', 'ぇ') => "tse",
        ('つ', 'ぉ') => "tso",
        _ => return None,
    })
}

fn kana_single(h: char) -> Option<&'static str> {
    Some(match h {
        'あ' | 'ぁ' => "a",
        'い' | 'ぃ' => "i",
        'う' | 'ぅ' => "u",
        'え' | 'ぇ' => "e",
        'お' | 'ぉ' => "o",
        'か' | 'ゕ' => "ka",
        'き' => "ki",
        'く' => "ku",
        'け' | 'ゖ' => "ke",
        'こ' => "ko",
        'さ' => "sa",
        'し' => "shi",
        'す' => "su",
        'せ' => "se",
        'そ' => "so",
        'た' => "ta",
        'ち' => "chi",
        'つ' => "tsu",
        'て' => "te",
        'と' => "to",
        'な' => "na",
        'に' => "ni",
        'ぬ' => "nu",
        'ね' => "ne",
        'の' => "no",
        'は' => "ha",
        'ひ' => "hi",
        'ふ' => "fu",
        'へ' => "he",
        'ほ' => "ho",
        'ま' => "ma",
        'み' => "mi",
        'む' => "mu",
        'め' => "me",
        'も' => "mo",
        'や' | 'ゃ' => "ya",
        'ゆ' | 'ゅ' => "yu",
        'よ' | 'ょ' => "yo",
        'ら' => "ra",
        'り' => "ri",
        'る' => "ru",
        'れ' => "re",
        'ろ' => "ro",
        'わ' | 'ゎ' => "wa",
        'ゐ' => "wi",
        'ゑ' => "we",
        'を' => "o",
        'ん' => "n",
        'が' => "ga",
        'ぎ' => "gi",
        'ぐ' => "gu",
        'げ' => "ge",
        'ご' => "go",
        'ざ' => "za",
        'じ' => "ji",
        'ず' => "zu",
        'ぜ' => "ze",
        'ぞ' => "zo",
        'だ' => "da",
        'ぢ' => "ji",
        'づ' => "zu",
        'で' => "de",
        'ど' => "do",
        'ば' => "ba",
        'び' => "bi",
        'ぶ' => "bu",
        'べ' => "be",
        'ぼ' => "bo",
        'ぱ' => "pa",
        'ぴ' => "pi",
        'ぷ' => "pu",
        'ぺ' => "pe",
        'ぽ' => "po",
        'ゔ' => "vu",
        '・' => " ",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Korean Hangul -> Revised Romanization (RR)
// ---------------------------------------------------------------------------

const HANGUL_BASE: u32 = 0xAC00;
const HANGUL_MEDIALS: u32 = 21;
const HANGUL_FINALS: u32 = 28;

const RR_INITIALS: [&str; 19] = [
    "g", "kk", "n", "d", "tt", "r", "m", "b", "pp", "s", "ss", "", "j", "jj", "ch", "k", "t", "p",
    "h",
];

const RR_MEDIALS: [&str; 21] = [
    "a", "ae", "ya", "yae", "eo", "e", "yeo", "ye", "o", "wa", "wae", "oe", "yo", "u", "wo", "we",
    "wi", "yu", "eu", "ui", "i",
];

const RR_FINALS: [&str; 28] = [
    "", "k", "k", "k", "n", "n", "n", "t", "l", "k", "m", "l", "l", "l", "p", "l", "m", "p", "p",
    "t", "t", "ng", "t", "t", "k", "t", "p", "t",
];

fn decompose_hangul(ch: char) -> Option<(usize, usize, usize)> {
    if !is_hangul_syllable(ch) {
        return None;
    }
    let idx = ch as u32 - HANGUL_BASE;
    let l = (idx / (HANGUL_MEDIALS * HANGUL_FINALS)) as usize;
    let v = ((idx % (HANGUL_MEDIALS * HANGUL_FINALS)) / HANGUL_FINALS) as usize;
    let t = (idx % HANGUL_FINALS) as usize;
    Some((l, v, t))
}

fn transliterate_hangul_word(chars: &[char], start: usize, out: &mut String) -> usize {
    let mut syllables = Vec::new();
    let mut i = start;
    while i < chars.len() {
        let Some(parts) = decompose_hangul(chars[i]) else {
            break;
        };
        syllables.push(parts);
        i += 1;
    }

    let mut onset_override: Option<&'static str> = None;
    for (idx, &(l, v, t)) in syllables.iter().enumerate() {
        let initial = onset_override.take().unwrap_or(RR_INITIALS[l]);
        out.push_str(initial);
        out.push_str(RR_MEDIALS[v]);

        let next = syllables.get(idx + 1).copied();
        let (final_rom, next_onset) = resolve_hangul_final(t, next);
        out.push_str(final_rom);
        onset_override = next_onset;
    }

    syllables.len()
}

fn resolve_hangul_final(
    t: usize,
    next: Option<(usize, usize, usize)>,
) -> (&'static str, Option<&'static str>) {
    if t == 0 {
        return ("", None);
    }
    let Some((next_l, _, _)) = next else {
        return (RR_FINALS[t], None);
    };

    // Vowel liaison when the following syllable begins with silent ㅇ (initial index 11).
    if next_l == 11 {
        return match t {
            1 => ("", Some("g")),
            2 => ("", Some("kk")),
            3 => ("k", Some("s")),
            4 => ("", Some("n")),
            5 => ("n", Some("j")),
            6 => ("n", None),
            7 => ("", Some("d")),
            8 => ("", Some("r")),
            9 => ("l", Some("g")),
            10 => ("l", Some("m")),
            11 => ("l", Some("b")),
            12 => ("l", Some("s")),
            13 => ("l", Some("t")),
            14 => ("l", Some("p")),
            15 => ("l", None),
            16 => ("", Some("m")),
            17 => ("", Some("b")),
            18 => ("p", Some("s")),
            19 => ("", Some("s")),
            20 => ("", Some("ss")),
            21 => ("ng", None),
            22 => ("", Some("j")),
            23 => ("", Some("ch")),
            24 => ("", Some("k")),
            25 => ("", Some("t")),
            26 => ("", Some("p")),
            27 => ("", None),
            _ => (RR_FINALS[t], None),
        };
    }

    // Nasalization before ㄴ (2) or ㅁ (6).
    if matches!(next_l, 2 | 6) {
        return match t {
            1 | 2 | 3 | 9 | 24 => ("ng", None),
            7 | 19 | 20 | 22 | 23 | 25 | 27 => ("n", None),
            14 | 17 | 18 | 26 => ("m", None),
            8 => ("l", Some("l")),
            _ => (RR_FINALS[t], None),
        };
    }

    // Liquid assimilation across ㄴ (4/2) and ㄹ (8/5).
    if (t == 4 && next_l == 5) || (t == 8 && next_l == 5) {
        return ("l", Some("l"));
    }

    (RR_FINALS[t], None)
}

fn transliterate_compat_jamo(ch: char) -> Option<&'static str> {
    Some(match ch {
        'ㄱ' => "g",
        'ㄲ' => "kk",
        'ㄴ' => "n",
        'ㄷ' => "d",
        'ㄸ' => "tt",
        'ㄹ' => "r",
        'ㅁ' => "m",
        'ㅂ' => "b",
        'ㅃ' => "pp",
        'ㅅ' => "s",
        'ㅆ' => "ss",
        'ㅇ' => "ng",
        'ㅈ' => "j",
        'ㅉ' => "jj",
        'ㅊ' => "ch",
        'ㅋ' => "k",
        'ㅌ' => "t",
        'ㅍ' => "p",
        'ㅎ' => "h",
        'ㅏ' => "a",
        'ㅐ' => "ae",
        'ㅑ' => "ya",
        'ㅒ' => "yae",
        'ㅓ' => "eo",
        'ㅔ' => "e",
        'ㅕ' => "yeo",
        'ㅖ' => "ye",
        'ㅗ' => "o",
        'ㅛ' => "yo",
        'ㅜ' => "u",
        'ㅠ' => "yu",
        'ㅡ' => "eu",
        'ㅣ' => "i",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Cyrillic & Greek Transliteration
// ---------------------------------------------------------------------------

fn push_cased(out: &mut String, base_lower: &str, is_upper: bool, neighbor_upper: bool) {
    if !is_upper {
        out.push_str(base_lower);
    } else if neighbor_upper {
        out.push_str(&base_lower.to_ascii_uppercase());
    } else {
        let mut chars = base_lower.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.push_str(chars.as_str());
        }
    }
}

fn is_cyrillic_iotating_prev(prev: Option<char>) -> bool {
    match prev {
        None => true,
        Some(c) if !c.is_alphabetic() => true,
        Some(c) => matches!(
            c.to_lowercase().next(),
            Some('а' | 'е' | 'ё' | 'и' | 'й' | 'о' | 'у' | 'ы' | 'э' | 'ю' | 'я' | 'ь' | 'ъ')
        ),
    }
}

fn transliterate_cyrillic_char(ch: char, prev: Option<char>, next: Option<char>, out: &mut String) {
    let is_upper = ch.is_uppercase();
    let neighbor_upper = next.is_some_and(|c| c.is_uppercase())
        || (prev.is_some_and(|c| c.is_uppercase()) && next.is_none_or(|c| !c.is_lowercase()));

    let lower = ch.to_lowercase().next().unwrap_or(ch);
    let mapped = match lower {
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "g",
        'д' => "d",
        'е' => {
            if is_cyrillic_iotating_prev(prev) {
                "ye"
            } else {
                "e"
            }
        }
        'ё' => "yo",
        'ж' => "zh",
        'з' => "z",
        'и' => "i",
        'й' => "y",
        'к' => "k",
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => "o",
        'п' => "p",
        'р' => "r",
        'с' => "s",
        'т' => "t",
        'у' => "u",
        'ф' => "f",
        'х' => "kh",
        'ц' => "ts",
        'ч' => "ch",
        'ш' => "sh",
        'щ' => "shch",
        'ъ' => "",
        'ы' => "y",
        'ь' => "'",
        'э' => "e",
        'ю' => "yu",
        'я' => "ya",
        // Ukrainian / Belarusian / Serbian / Kazakh common letters
        'ґ' => "g",
        'є' => "ye",
        'і' => "i",
        'ї' => "yi",
        'ў' => "w",
        'ђ' => "dj",
        'ј' => "j",
        'љ' => "lj",
        'њ' => "nj",
        'ћ' => "c",
        'џ' | 'ѕ' => "dz",
        'ѓ' => "gj",
        'ќ' => "kj",
        'ә' => "a",
        'ө' => "o",
        'ү' | 'ұ' => "u",
        'қ' => "q",
        'ғ' => "gh",
        'ң' => "ng",
        'һ' => "h",
        _ => {
            out.push(ch);
            return;
        }
    };

    push_cased(out, mapped, is_upper, neighbor_upper);
}

fn greek_base_lower(ch: char) -> char {
    let lower = ch.to_lowercase().next().unwrap_or(ch);
    match lower {
        'ά' | 'ἀ' | 'ἁ' | 'ἂ' | 'ἃ' | 'ἄ' | 'ἅ' | 'ἆ' | 'ἇ' | 'ὰ' | 'ᾀ' | 'ᾁ' | 'ᾴ' | 'ᾶ' | 'ᾷ' => {
            'α'
        }
        'έ' | 'ἐ' | 'ἑ' | 'ἒ' | 'ἓ' | 'ἔ' | 'ἕ' | 'ὲ' => 'ε',
        'ή' | 'ἠ' | 'ἡ' | 'ἢ' | 'ἣ' | 'ἤ' | 'ἥ' | 'ἦ' | 'ἧ' | 'ὴ' | 'ᾐ' | 'ᾑ' | 'ῄ' | 'ῆ' | 'ῇ' => {
            'η'
        }
        'ί' | 'ϊ' | 'ΐ' | 'ἰ' | 'ἱ' | 'ἲ' | 'ἳ' | 'ἴ' | 'ἵ' | 'ἶ' | 'ἷ' | 'ὶ' | 'ῖ' => {
            'ι'
        }
        'ό' | 'ὀ' | 'ὁ' | 'ὂ' | 'ὃ' | 'ὄ' | 'ὅ' | 'ὸ' => 'ο',
        'ύ' | 'ϋ' | 'ΰ' | 'ὐ' | 'ὑ' | 'ὒ' | 'ὓ' | 'ὔ' | 'ὕ' | 'ὖ' | 'ὗ' | 'ὺ' | 'ῦ' => {
            'υ'
        }
        'ώ' | 'ὠ' | 'ὡ' | 'ὢ' | 'ὣ' | 'ὤ' | 'ὥ' | 'ὦ' | 'ὧ' | 'ὼ' | 'ᾠ' | 'ᾡ' | 'ῴ' | 'ῶ' | 'ῷ' => {
            'ω'
        }
        'ῥ' | 'ῤ' => 'ρ',
        'ς' => 'σ',
        other => other,
    }
}

fn transliterate_greek_at(
    chars: &[char],
    idx: usize,
    prev: Option<char>,
    next: Option<char>,
    out: &mut String,
) -> usize {
    let ch = chars[idx];
    let is_upper = ch.is_uppercase();
    let word_start = prev.is_none_or(|c| !c.is_alphabetic());

    if let Some(next_ch) = next {
        let b1 = greek_base_lower(ch);
        let b2 = greek_base_lower(next_ch);
        let digraph = match (b1, b2) {
            ('ο', 'υ') => Some("ou"),
            ('μ', 'π') if word_start => Some("b"),
            ('ν', 'τ') if word_start => Some("d"),
            ('γ', 'κ') if word_start => Some("g"),
            ('γ', 'γ') => Some("ng"),
            ('γ', 'ξ') => Some("nx"),
            ('γ', 'χ') => Some("nch"),
            _ => None,
        };
        if let Some(rom) = digraph {
            let after_pair = chars.get(idx + 2).copied();
            let neighbor_upper =
                next_ch.is_uppercase() || after_pair.is_some_and(|c| c.is_uppercase());
            push_cased(out, rom, is_upper, neighbor_upper);
            return 2;
        }
    }

    let neighbor_upper = next.is_some_and(|c| c.is_uppercase())
        || (prev.is_some_and(|c| c.is_uppercase()) && next.is_none_or(|c| !c.is_lowercase()));

    let mapped = match greek_base_lower(ch) {
        'α' => "a",
        'β' => "v",
        'γ' => "g",
        'δ' => "d",
        'ε' => "e",
        'ζ' => "z",
        'η' => "i",
        'θ' => "th",
        'ι' => "i",
        'κ' => "k",
        'λ' => "l",
        'μ' => "m",
        'ν' => "n",
        'ξ' => "x",
        'ο' => "o",
        'π' => "p",
        'ρ' => "r",
        'σ' => "s",
        'τ' => "t",
        'υ' => "y",
        'φ' => "f",
        'χ' => "ch",
        'ψ' => "ps",
        'ω' => "o",
        _ => {
            out.push(ch);
            return 1;
        }
    };

    push_cased(out, mapped, is_upper, neighbor_upper);
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::LyricLine;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::TcpListener;

    fn temp_core_paths(label: &str) -> (PathBuf, CorePaths) {
        let unique = format!(
            "sonata_translate_test_{label}_{}_{}",
            std::process::id(),
            TEMP_FILE_SEQ.fetch_add(1, Ordering::Relaxed)
        );
        let root = std::env::temp_dir().join(unique);
        let paths = CorePaths::for_app_roots(root.join("cfg"), root.join("cache"));
        (root, paths)
    }

    async fn spawn_mock_translate_server<F>(handler: F) -> (String, Arc<AtomicUsize>)
    where
        F: Fn(&str, usize) -> (u16, String) + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_bg = Arc::clone(&hits);
        let handler = Arc::new(handler);

        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let hits_req = Arc::clone(&hits_bg);
                let handler_req = Arc::clone(&handler);
                tokio::spawn(async move {
                    let mut buf = Vec::with_capacity(4096);
                    let mut chunk = [0u8; 2048];
                    let mut header_end = None;
                    let mut content_len = 0usize;

                    loop {
                        let Ok(n) = stream.read(&mut chunk).await else {
                            return;
                        };
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                        if header_end.is_none() {
                            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                                let hdr_end = pos + 4;
                                header_end = Some(hdr_end);
                                let headers = String::from_utf8_lossy(&buf[..pos]);
                                for line in headers.lines() {
                                    if let Some((name, val)) = line.split_once(':') {
                                        if name.eq_ignore_ascii_case("content-length") {
                                            content_len = val.trim().parse().unwrap_or(0);
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(hdr_end) = header_end {
                            if buf.len() >= hdr_end + content_len {
                                break;
                            }
                        }
                    }

                    let body = header_end
                        .map(|hdr_end| {
                            String::from_utf8_lossy(
                                &buf[hdr_end..buf.len().min(hdr_end + content_len)],
                            )
                            .into_owned()
                        })
                        .unwrap_or_default();
                    let call_idx = hits_req.fetch_add(1, AtomicOrdering::SeqCst);
                    let (status, resp_body) = handler_req(&body, call_idx);
                    let reason = if status == 200 { "OK" } else { "Error" };
                    let raw_resp = format!(
                        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{resp_body}",
                        resp_body.len()
                    );
                    let _ = stream.write_all(raw_resp.as_bytes()).await;
                    let _ = stream.shutdown().await;
                });
            }
        });

        (format!("http://{addr}/translate_a/single"), hits)
    }

    fn extract_form_param(form_body: &str, key: &str) -> Option<String> {
        for pair in form_body.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                if k == key {
                    return urlencoding::decode(v).ok().map(|s| s.into_owned());
                }
            }
        }
        None
    }

    #[test]
    fn romanizes_japanese_hiragana_katakana_and_preserves_kanji_awareness() {
        assert_eq!(romanize_text("さくら さくら"), "sakura sakura");
        assert_eq!(romanize_text("さようなら"), "sayounara");
        assert_eq!(romanize_text("まっちゃ きっぷ"), "matcha kippu");
        assert_eq!(romanize_text("しんや あんない"), "shin'ya annai");
        assert_eq!(romanize_text("コーヒー スター"), "koohii sutaa");
        assert_eq!(romanize_text("ヴァイオリン チェック"), "vaiorin chekku");
        // Halfwidth katakana with voiced mark
        assert_eq!(romanize_text("ﾃﾞｨｽｺ ﾊﾟｰﾃｨ"), "disuko paati");

        let mixed = romanize_text_with_report("東京の夜");
        assert!(mixed.has_untransliterated_han);
        assert!(mixed.changed);
        assert_eq!(mixed.text, "東京no夜");

        let pure_kanji = romanize_text_with_report("你好世界");
        assert!(pure_kanji.has_untransliterated_han);
        assert!(!pure_kanji.changed);
        assert_eq!(pure_kanji.text, "你好世界");
    }

    #[test]
    fn romanizes_korean_hangul_revised_romanization() {
        assert_eq!(romanize_text("사랑해"), "saranghae");
        assert_eq!(romanize_text("안녕하세요"), "annyeonghaseyo");
        assert_eq!(romanize_text("한국어"), "hangugeo");
        assert_eq!(romanize_text("감사합니다"), "gamsahamnida");
        assert_eq!(romanize_text("신라"), "silla");
        assert_eq!(romanize_text("좋아"), "joa");
        assert_eq!(romanize_text("ㅋㅋㅋ"), "kkk");
    }

    #[test]
    fn romanizes_cyrillic_and_greek_preserving_case() {
        assert_eq!(romanize_text("Привет мир"), "Privet mir");
        assert_eq!(romanize_text("МОСКВА"), "MOSKVA");
        assert_eq!(romanize_text("Жизнь и любовь"), "Zhizn' i lyubov'");
        assert_eq!(romanize_text("Щука"), "Shchuka");
        assert_eq!(romanize_text("Αθήνα Ελλάδα"), "Athina Ellada");
        assert_eq!(romanize_text("ΜΠΕΜΠΗΣ"), "BEMPIS");
        assert_eq!(romanize_text("Θάλασσα ουρανός"), "Thalassa ouranos");
    }

    #[test]
    fn skips_latin_only_and_empty_inputs_for_offline_romanization() {
        assert!(!contains_non_latin_letter(&[
            "Hello world!".to_string(),
            "Şarkı sözü — 123".to_string(),
            "Café résumé".to_string(),
        ]));
        assert!(contains_non_latin_letter(&[
            "Hello".to_string(),
            "사랑해".to_string(),
        ]));

        assert_eq!(romanize_lines_offline(&[]), None);
        assert_eq!(
            romanize_lines_offline(&["Hello world".to_string(), "123!".to_string()]),
            None
        );
        assert_eq!(
            romanize_lines_offline(&["   ".to_string(), "".to_string()]),
            Some(vec!["   ".to_string(), "".to_string()])
        );
        // Pure Han/Kanji has no deterministic offline mapping, so offline romanizer returns None.
        assert_eq!(romanize_lines_offline(&["你好世界".to_string()]), None);
    }

    #[test]
    fn chunks_batches_and_parses_pua_delimiters_with_realignment() {
        let long_a = "あ".repeat(2_000);
        let long_b = "い".repeat(1_500);
        let lines = vec![
            "".to_string(),
            long_a.clone(),
            "   ".to_string(),
            long_b.clone(),
            "ううう".to_string(),
        ];

        let slots = collect_slots(&lines);
        assert_eq!(slots.len(), 3);
        let batches = build_batches(&slots);
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].slots.len(), 1);
        assert_eq!(batches[1].slots.len(), 2);
        assert!(batches[1].payload.contains("\u{E000}0001\u{E001}"));

        // Simulate CJK endpoint normalizing closing U+E001 to U+E000.
        let fake_json =
            serde_json::json!([[["line-b\n\u{E000}0001\u{E000}\nline-c", null, null, null]]])
                .to_string();
        let parsed = parse_translate_response(&fake_json, 2, false).unwrap();
        assert_eq!(parsed, vec!["line-b", "line-c"]);

        let merged = merge_back(&lines, &slots, vec!["A".into(), "B".into(), "C".into()]);
        assert_eq!(merged, vec!["", "A", "   ", "B", "C"]);
    }

    #[test]
    fn extracts_lines_from_synced_and_plain_lyrics_state() {
        let synced = LyricsState::Synced(vec![
            LyricLine::new(1.0, "첫 번째 줄"),
            LyricLine::new(2.0, ""),
            LyricLine::new(3.0, "두 번째 줄"),
        ]);
        assert_eq!(
            extract_lyrics_lines(&synced),
            Some(vec![
                "첫 번째 줄".to_string(),
                "".to_string(),
                "두 번째 줄".to_string(),
            ])
        );
        let plain = LyricsState::Plain(vec!["One".into(), "Two".into()]);
        assert_eq!(
            extract_lyrics_lines(&plain),
            Some(vec!["One".into(), "Two".into()])
        );
        assert_eq!(extract_lyrics_lines(&LyricsState::Idle), None);
        assert_eq!(extract_lyrics_lines(&LyricsState::Loading), None);
        assert_eq!(extract_lyrics_lines(&LyricsState::NotFound), None);
    }

    #[test]
    fn persists_and_invalidates_json_cache_with_private_permissions() {
        let (root, paths) = temp_core_paths("cache_roundtrip");
        let src = vec!["첫 줄".to_string(), "".to_string(), "둘째 줄".to_string()];
        let out = vec![
            "First line".to_string(),
            "".to_string(),
            "Second line".to_string(),
        ];

        save_cached_lyrics_lines(&paths, "track-42", "translate", "EN", &src, &out).unwrap();
        let loaded = load_cached_lyrics_lines(&paths, "track-42", "translate", "en", &src);
        assert_eq!(loaded.as_ref(), Some(&out));

        // Different language or modified source lines must not collide or misalign.
        assert_eq!(
            load_cached_lyrics_lines(&paths, "track-42", "translate", "tr", &src),
            None
        );
        let changed_src = vec![
            "첫 줄".to_string(),
            "새 줄".to_string(),
            "둘째 줄".to_string(),
        ];
        assert_eq!(
            load_cached_lyrics_lines(&paths, "track-42", "translate", "en", &changed_src),
            None
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let dir_mode = fs::metadata(paths.lyrics_cache_dir())
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            let file_mode =
                fs::metadata(translate_cache_path(&paths, "track-42", "translate", "en"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777;
            assert_eq!(dir_mode, 0o700);
            assert_eq!(file_mode, 0o600);
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn translates_in_batches_preserves_alignment_and_hits_disk_cache() {
        let (root, paths) = temp_core_paths("async_translate");
        let (endpoint, hits) = spawn_mock_translate_server(|body, _idx| {
            let dt = extract_form_param(body, "dt").unwrap_or_default();
            let q = extract_form_param(body, "q").unwrap_or_default();
            assert_eq!(dt, "t");
            let re = Regex::new(r"\u{E000}[^\u{E000}\u{E001}]*\u{E001}").unwrap();
            let mut out = String::new();
            let mut last = 0;
            for (n, m) in re.find_iter(&q).enumerate() {
                let seg = q[last..m.start()].trim();
                out.push_str(&format!("TR:{seg}"));
                out.push('\n');
                out.push_str(m.as_str());
                out.push('\n');
                last = m.end();
                let _ = n;
            }
            let tail = q[last..].trim();
            out.push_str(&format!("TR:{tail}"));

            let json = serde_json::json!([[[out, q, null, null]]]).to_string();
            (200, json)
        })
        .await;

        let filler = "a".repeat(2_000);
        let filler_b = "b".repeat(1_500);
        let lines = vec![
            "".to_string(),
            filler.clone(),
            "   ".to_string(),
            filler_b.clone(),
            "troisième ligne".to_string(),
        ];

        let http = Client::new();
        let translated = translate_lyrics_lines_with_endpoint(
            &http,
            &paths,
            "yt:abc123",
            &lines,
            "en",
            &endpoint,
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(translated.len(), lines.len());
        assert_eq!(translated[0], "");
        assert_eq!(translated[1], format!("TR:{filler}"));
        assert_eq!(translated[2], "   ");
        assert_eq!(translated[3], format!("TR:{filler_b}"));
        assert_eq!(translated[4], "TR:troisième ligne");
        assert_eq!(hits.load(AtomicOrdering::SeqCst), 2);

        // Second call must hit the on-disk JSON cache without making HTTP requests.
        let cached = translate_lyrics_lines_with_endpoint(
            &http,
            &paths,
            "yt:abc123",
            &lines,
            "en",
            &endpoint,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(cached, translated);
        assert_eq!(hits.load(AtomicOrdering::SeqCst), 2);

        // Empty target or empty lines returns Ok(None).
        assert_eq!(
            translate_lyrics_lines_with_endpoint(
                &http,
                &paths,
                "yt:abc123",
                &lines,
                "  ",
                &endpoint
            )
            .await
            .unwrap(),
            None
        );
        assert_eq!(
            translate_lyrics_lines_with_endpoint(&http, &paths, "yt:abc123", &[], "en", &endpoint)
                .await
                .unwrap(),
            None
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn romanize_skips_latin_tracks_and_uses_kanji_aware_fallback() {
        let (root, paths) = temp_core_paths("async_romanize");
        let http = Client::new();

        // Latin-only lyrics must skip network requests completely.
        let latin_only = vec!["Already in Latin".to_string(), "Café".to_string()];
        assert_eq!(
            romanize_lyrics_lines_with_endpoint(
                &http,
                &paths,
                "track-latin",
                &latin_only,
                "http://127.0.0.1:1/unreachable",
            )
            .await
            .unwrap(),
            None
        );

        // Hangul lyrics offline fallback succeeds without Han/Kanji.
        let korean = vec![
            "사랑해".to_string(),
            "".to_string(),
            "안녕하세요".to_string(),
        ];
        let rom_ko = romanize_lyrics_lines_with_endpoint(
            &http,
            &paths,
            "track-ko",
            &korean,
            "http://127.0.0.1:1/unreachable",
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(rom_ko, vec!["saranghae", "", "annyeonghaseyo"]);

        // Kanji-containing Japanese must NOT pretend to offline-romanize kanji when endpoint fails.
        let kanji_jp = vec!["東京の夜".to_string()];
        let err = romanize_lyrics_lines_with_endpoint(
            &http,
            &paths,
            "track-jp",
            &kanji_jp,
            "http://127.0.0.1:1/unreachable",
        )
        .await
        .unwrap_err();
        assert!(matches!(
            err,
            CoreError::Offline { .. } | CoreError::Io { .. } | CoreError::Internal { .. }
        ));

        // When the endpoint succeeds with dt=rm, kanji lyrics are romanized via segment index 3.
        let (endpoint, hits) = spawn_mock_translate_server(|body, _| {
            assert_eq!(extract_form_param(body, "dt").as_deref(), Some("rm"));
            let json = serde_json::json!([[["", "東京の夜", null, "Tokyo no yoru"]]]).to_string();
            (200, json)
        })
        .await;

        let rom_jp =
            romanize_lyrics_lines_with_endpoint(&http, &paths, "track-jp", &kanji_jp, &endpoint)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(rom_jp, vec!["Tokyo no yoru"]);
        assert_eq!(hits.load(AtomicOrdering::SeqCst), 1);

        let _ = fs::remove_dir_all(&root);
    }
}

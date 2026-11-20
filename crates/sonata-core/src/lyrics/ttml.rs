//! TTML (Timed Text Markup Language) lyrics parser for Sonata.
//!
//! 1. **Agent & Voice Mapping**: reads `<ttm:agent type="person" xml:id="..."/>` declarations and maps
//!    the first person agent on `<p>` elements to `LyricVoice::Lead` and any additional person agents
//!    to `LyricVoice::Counter` for left/right duet alignment.
//! 2. **Word-Level Timing**: extracts `<span begin="..." end="...">` word/syllable timings into
//!    `Vec<LyricWord>`, merging adjacent non-whitespace-separated syllables so words remain intact.
//! 3. **Background & Aside Handling**: emits `<span ttm:role="x-bg">` spans as parenthesized
//!    background `LyricLine` entries while skipping `x-translation` and `x-roman` spans.
//! 4. **Timestamp Parsing**: supports `hh:mm:ss.xxx`, `mm:ss.xxx`, `ss.xxxs`, and `ms` formats.

use crate::lyrics::{LyricLine, LyricVoice, LyricWord};
use roxmltree::{Document, Node};
use std::collections::HashMap;

const TTM_NS: &str = "http://www.w3.org/ns/ttml#metadata";
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";
const BG_ROLE: &str = "x-bg";
const SKIP_ROLES: [&str; 2] = ["x-translation", "x-roman"];

/// Parses a TTML XML document into time-synced `LyricLine`s.
/// Returns `None` if XML parsing fails or no timed lines are present.
pub fn parse_ttml(xml: &str) -> Option<Vec<LyricLine>> {
    let doc = Document::parse(xml).ok()?;
    let person_agents = collect_person_agents(&doc);
    let lead_agent = find_lead_agent(&doc, &person_agents);

    let mut lines = Vec::new();
    for p in paragraphs(&doc) {
        parse_paragraph(p, lead_agent, &person_agents, &mut lines);
    }

    let has_timing = lines
        .iter()
        .any(|l| l.time_secs > 0.0 || !l.words.is_empty());
    if lines.is_empty() || !has_timing {
        return None;
    }

    lines.sort_by(|a, b| a.time_secs.total_cmp(&b.time_secs));
    Some(lines)
}

fn paragraphs<'a>(doc: &'a Document<'a>) -> impl Iterator<Item = Node<'a, 'a>> {
    doc.descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "p")
}

fn collect_person_agents<'a>(doc: &'a Document<'a>) -> HashMap<&'a str, bool> {
    doc.descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "agent")
        .filter_map(|node| {
            let id = node
                .attribute((XML_NS, "id"))
                .or_else(|| node.attribute("id"))?;
            Some((id, node.attribute("type") == Some("person")))
        })
        .collect()
}

fn agent_attr<'a>(node: Node<'a, 'a>) -> Option<&'a str> {
    node.attribute((TTM_NS, "agent"))
        .or_else(|| node.attribute("agent"))
}

fn role_attr<'a>(node: Node<'a, 'a>) -> Option<&'a str> {
    node.attribute((TTM_NS, "role"))
        .or_else(|| node.attribute("role"))
}

fn find_lead_agent<'a>(
    doc: &'a Document<'a>,
    person_agents: &HashMap<&str, bool>,
) -> Option<&'a str> {
    paragraphs(doc)
        .filter_map(agent_attr)
        .find(|agent| person_agents.get(agent).copied().unwrap_or(false))
}

fn resolve_voice(
    p: Node<'_, '_>,
    lead_agent: Option<&str>,
    person_agents: &HashMap<&str, bool>,
) -> LyricVoice {
    let Some(agent) = agent_attr(p) else {
        return LyricVoice::Lead;
    };
    let is_person = person_agents.get(agent).copied().unwrap_or(false);
    if is_person && Some(agent) != lead_agent {
        LyricVoice::Counter
    } else {
        LyricVoice::Lead
    }
}

#[derive(Debug, Clone)]
struct SpanChunk {
    start_secs: f64,
    end_secs: Option<f64>,
    text: String,
    separated_from_prev: bool,
}

fn collect_spans(
    parent: Node<'_, '_>,
    words: &mut Vec<SpanChunk>,
    bg_lines: &mut Vec<LyricLine>,
    pending_space: &mut bool,
    voice: LyricVoice,
) {
    for child in parent.children() {
        if child.is_text() {
            let raw = child.text().unwrap_or_default();
            if !raw.is_empty() && raw.chars().any(char::is_whitespace) {
                *pending_space = true;
            }
            if !raw.trim().is_empty() {
                if let Some(last) = words.last_mut() {
                    if *pending_space && !last.text.ends_with(char::is_whitespace) {
                        // Trailing text attached outside timed span.
                        last.text.push(' ');
                    }
                    last.text.push_str(raw.trim());
                    *pending_space = raw.ends_with(char::is_whitespace);
                }
            }
            continue;
        }
        if !child.is_element() {
            continue;
        }
        match role_attr(child) {
            Some(BG_ROLE) => {
                if let Some(bg) = parse_background_span(child, voice) {
                    bg_lines.push(bg);
                }
            }
            Some(role) if SKIP_ROLES.contains(&role) => {}
            _ if child.has_attribute("begin") => {
                if let Some(start_secs) = parse_attr_stamp(child, "begin") {
                    let end_secs = parse_attr_stamp(child, "end").map(|e| e.max(start_secs));
                    let raw_text: String = child
                        .descendants()
                        .filter(Node::is_text)
                        .filter(|n| !has_skipped_role(*n))
                        .filter_map(|n| n.text())
                        .collect();
                    let starts_ws = raw_text.starts_with(char::is_whitespace);
                    let ends_ws = raw_text.ends_with(char::is_whitespace);
                    let trimmed = raw_text.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !trimmed.is_empty() {
                        words.push(SpanChunk {
                            start_secs,
                            end_secs,
                            text: trimmed,
                            separated_from_prev: *pending_space || starts_ws || words.is_empty(),
                        });
                        *pending_space = ends_ws;
                    } else if ends_ws || starts_ws {
                        *pending_space = true;
                    }
                }
            }
            _ => {
                collect_spans(child, words, bg_lines, pending_space, voice);
            }
        }
    }
}

fn has_skipped_role(node: Node<'_, '_>) -> bool {
    node.ancestors()
        .filter_map(role_attr)
        .any(|r| SKIP_ROLES.contains(&r))
}

fn merge_syllable_chunks(chunks: Vec<SpanChunk>) -> Vec<LyricWord> {
    let mut out: Vec<LyricWord> = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        if !chunk.separated_from_prev {
            if let Some(prev) = out.last_mut() {
                prev.text.push_str(&chunk.text);
                if let Some(end) = chunk.end_secs {
                    prev.end_secs = Some(prev.end_secs.map_or(end, |e| e.max(end)));
                }
                continue;
            }
        }
        out.push(LyricWord {
            start_secs: chunk.start_secs,
            end_secs: chunk.end_secs,
            text: chunk.text,
        });
    }
    out
}

fn parse_background_span(span: Node<'_, '_>, voice: LyricVoice) -> Option<LyricLine> {
    let mut chunks = Vec::new();
    let mut ignored_nested_bg = Vec::new();
    let mut pending_space = true;
    collect_spans(
        span,
        &mut chunks,
        &mut ignored_nested_bg,
        &mut pending_space,
        voice,
    );
    let mut words = merge_syllable_chunks(chunks);
    let mut text = if words.is_empty() {
        plain_text_excluding_asides(span, false)
    } else {
        words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if !(trimmed.starts_with('(') && trimmed.ends_with(')')) {
        text = format!("({trimmed})");
        if let Some(first) = words.first_mut() {
            if !first.text.starts_with('(') {
                first.text.insert(0, '(');
            }
        }
        if let Some(last) = words.last_mut() {
            if !last.text.ends_with(')') {
                last.text.push(')');
            }
        }
    } else {
        text = trimmed.to_string();
    }

    let time_secs =
        parse_attr_stamp(span, "begin").or_else(|| words.first().map(|w| w.start_secs))?;

    Some(LyricLine {
        time_secs,
        text,
        words,
        voice,
    })
}

fn plain_text_excluding_asides(node: Node<'_, '_>, exclude_bg: bool) -> String {
    node.descendants()
        .filter(Node::is_text)
        .filter(|n| {
            !n.ancestors()
                .filter_map(role_attr)
                .any(|role| (exclude_bg && role == BG_ROLE) || SKIP_ROLES.contains(&role))
        })
        .filter_map(|n| n.text())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_paragraph(
    p: Node<'_, '_>,
    lead_agent: Option<&str>,
    person_agents: &HashMap<&str, bool>,
    out: &mut Vec<LyricLine>,
) {
    let voice = resolve_voice(p, lead_agent, person_agents);
    let mut chunks = Vec::new();
    let mut bg_lines = Vec::new();
    let mut pending_space = true;
    collect_spans(p, &mut chunks, &mut bg_lines, &mut pending_space, voice);

    let words = merge_syllable_chunks(chunks);
    let text = if words.is_empty() {
        plain_text_excluding_asides(p, true)
    } else {
        words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    };

    if !text.is_empty() {
        let time_secs = parse_attr_stamp(p, "begin")
            .or_else(|| words.first().map(|w| w.start_secs))
            .unwrap_or(0.0);
        if parse_attr_stamp(p, "begin").is_some() || !words.is_empty() {
            out.push(LyricLine {
                time_secs,
                text,
                words,
                voice,
            });
        }
    }

    out.extend(bg_lines);
}

fn parse_attr_stamp(node: Node<'_, '_>, attr: &str) -> Option<f64> {
    parse_ttml_timestamp(node.attribute(attr)?)
}

/// Parses TTML timestamps (`hh:mm:ss.xxx`, `mm:ss.xxx`, `ss.xxxs`, `ms`).
pub fn parse_ttml_timestamp(raw: &str) -> Option<f64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(ms) = trimmed.strip_suffix("ms") {
        let millis: f64 = ms.trim().parse().ok()?;
        return (millis.is_finite() && millis >= 0.0).then_some(millis / 1000.0);
    }
    let clean = trimmed.strip_suffix('s').unwrap_or(trimmed).trim();
    let mut seconds = 0.0f64;
    let mut part_count = 0usize;
    for part in clean.split(':') {
        let val: f64 = part.trim().parse().ok()?;
        if !val.is_finite() || val < 0.0 {
            return None;
        }
        seconds = seconds * 60.0 + val;
        part_count += 1;
    }
    if part_count == 0 || part_count > 3 || !seconds.is_finite() {
        return None;
    }
    Some(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ttml_timestamp_formats() {
        assert!((parse_ttml_timestamp("01:02:03.500").unwrap() - 3723.5).abs() < 0.001);
        assert!((parse_ttml_timestamp("02:15.250").unwrap() - 135.25).abs() < 0.001);
        assert!((parse_ttml_timestamp("12.345s").unwrap() - 12.345).abs() < 0.001);
        assert!((parse_ttml_timestamp("2500ms").unwrap() - 2.5).abs() < 0.001);
        assert_eq!(parse_ttml_timestamp(""), None);
        assert_eq!(parse_ttml_timestamp("invalid"), None);
    }

    #[test]
    fn parses_word_timings_dual_voice_and_background_vocals() {
        let xml = r#"
        <tt xmlns="http://www.w3.org/ns/ttml"
            xmlns:ttm="http://www.w3.org/ns/ttml#metadata"
            xml:lang="en">
          <head>
            <metadata>
              <ttm:agent type="person" xml:id="v1"/>
              <ttm:agent type="person" xml:id="v2"/>
            </metadata>
          </head>
          <body>
            <div>
              <p begin="10.000" end="13.000" ttm:agent="v1">
                <span begin="10.000" end="10.500">Hel</span><span begin="10.500" end="11.000">lo</span>
                <span begin="11.200" end="12.000">world</span>
                <span ttm:role="x-bg" begin="12.100" end="12.900">
                  <span begin="12.100" end="12.500">oh</span>
                  <span begin="12.500" end="12.900">yeah</span>
                </span>
                <span ttm:role="x-translation">ignored translation</span>
              </p>
              <p begin="00:14.000" end="00:16.000" ttm:agent="v2">
                <span begin="00:14.000" end="00:15.000">Counter</span>
                <span begin="00:15.000" end="00:16.000">line</span>
              </p>
            </div>
          </body>
        </tt>
        "#;

        let lines = parse_ttml(xml).expect("valid TTML");
        assert_eq!(lines.len(), 3);

        assert_eq!(lines[0].voice, LyricVoice::Lead);
        assert_eq!(lines[0].text, "Hello world");
        assert_eq!(lines[0].words.len(), 2);
        assert_eq!(lines[0].words[0].text, "Hello");
        assert!((lines[0].words[0].start_secs - 10.0).abs() < 0.001);
        assert!((lines[0].words[0].end_secs.unwrap() - 11.0).abs() < 0.001);
        assert_eq!(lines[0].words[1].text, "world");

        assert_eq!(lines[1].text, "(oh yeah)");
        assert!((lines[1].time_secs - 12.1).abs() < 0.001);
        assert_eq!(lines[1].words.len(), 2);
        assert_eq!(lines[1].words[0].text, "(oh");
        assert_eq!(lines[1].words[1].text, "yeah)");

        assert_eq!(lines[2].voice, LyricVoice::Counter);
        assert_eq!(lines[2].text, "Counter line");
        assert!((lines[2].time_secs - 14.0).abs() < 0.001);
    }
}

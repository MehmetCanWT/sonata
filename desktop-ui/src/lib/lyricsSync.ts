import type { TimedLine } from "@/types/sonata";

export interface KaraokeToken {
  text: string;
  startMs: number;
  endMs: number;
}

/**
 * Small perceptual lead so letter-by-letter illumination aligns with vocal onset
 * without jumping early or lagging behind.
 */
export const LYRIC_LOOKAHEAD_MS = 35;

export function isParentheticalText(text: string): boolean {
  const trimmed = text.trim();
  return trimmed.startsWith("(") && trimmed.endsWith(")") && trimmed.length > 2;
}

export function isCounterSpeaker(line: TimedLine): boolean {
  return line.speaker === "Counter" || line.speaker === "Backing";
}

export function isBackgroundLine(line: TimedLine): boolean {
  return line.background || isParentheticalText(line.text);
}

/**
 * Resolves the primary scroll-anchor lyric index for `posMs`.
 * Keeps an active Lead verse anchored while an overlapping `(...)` background vocal
 * begins mid-verse so scroll does not jump prematurely.
 */
export function resolveActiveLineIndex(lines: TimedLine[], posMs: number): number {
  const effectiveMs = posMs + LYRIC_LOOKAHEAD_MS;
  let activeIdx = -1;
  for (let i = 0; i < lines.length; i++) {
    if (effectiveMs >= lines[i].start_ms) {
      activeIdx = i;
    } else {
      break;
    }
  }
  while (activeIdx > 0 && isBackgroundLine(lines[activeIdx])) {
    const prev = lines[activeIdx - 1];
    if (isBackgroundLine(prev)) {
      break;
    }
    if (effectiveMs < prev.end_ms) {
      activeIdx -= 1;
    } else {
      break;
    }
  }
  return activeIdx;
}

/**
 * Returns `true` when `lines[index]` should render as actively glowing/sung at `posMs`,
 * allowing duet lines and parenthesized `(...)` vocals to glow concurrently with the lead line.
 */
export function isLyricLineActive(
  lines: TimedLine[],
  index: number,
  activeIdx: number,
  posMs: number,
): boolean {
  if (index === activeIdx) return true;
  const line = lines[index];
  if (!line) return false;
  const effectiveMs = posMs + LYRIC_LOOKAHEAD_MS;
  const bg = isBackgroundLine(line);
  const counter = isCounterSpeaker(line);
  const tailPad = bg ? 420 : 240;
  if (effectiveMs < line.start_ms || effectiveMs > line.end_ms + tailPad) {
    return false;
  }
  if (bg || counter || line.words.length > 0) {
    return true;
  }
  const activeLine = activeIdx >= 0 ? lines[activeIdx] : undefined;
  return Boolean(
    activeLine && (isBackgroundLine(activeLine) || isCounterSpeaker(activeLine)),
  );
}

/**
 * Returns `[0, 1]` sweep progress for a single word/syllable token.
 */
export function computeTokenProgress(posMs: number, startMs: number, endMs: number): number {
  const effectiveMs = posMs + LYRIC_LOOKAHEAD_MS;
  if (effectiveMs <= startMs) return 0;
  if (effectiveMs >= endMs) return 1;
  return (effectiveMs - startMs) / Math.max(80, endMs - startMs);
}

function charWeight(word: string): number {
  const trimmed = word.trim();
  const base = Math.max(1, trimmed.length);
  const last = trimmed.slice(-1);
  if (last === "," || last === ";" || last === ":") return base + 0.55;
  if (last === "." || last === "!" || last === "?") return base + 0.85;
  return base;
}

/**
 * Builds normalized word timing tokens for either word-synced (`words.length > 0`)
 * or line-synced verses, synthesizing smooth cadence windows when needed.
 */
export function buildKaraokeTokens(
  line: TimedLine,
  nextLineStartMs?: number,
): KaraokeToken[] {
  if (line.words.length > 0) {
    const out: KaraokeToken[] = [];
    for (let i = 0; i < line.words.length; i++) {
      const w = line.words[i];
      const nextWord = line.words[i + 1];
      const prevEnd = out.length > 0 ? out[out.length - 1].endMs : line.start_ms;
      const startMs = Math.max(
        w.start_ms,
        out.length > 0 ? Math.min(w.start_ms, prevEnd) : w.start_ms,
      );
      let endMs = w.end_ms;
      if (endMs <= startMs + 40) {
        if (nextWord && nextWord.start_ms > startMs) {
          endMs = nextWord.start_ms;
        } else if (nextLineStartMs && nextLineStartMs > startMs) {
          endMs = Math.min(nextLineStartMs - 50, startMs + 2200);
        } else {
          endMs = startMs + 420;
        }
      }
      out.push({
        text: w.text,
        startMs,
        endMs: Math.max(endMs, startMs + 110),
      });
    }
    return out;
  }

  const rawWords = line.text.trim().split(/\s+/).filter(Boolean);
  if (rawWords.length === 0) return [];

  const lineStart = line.start_ms;
  const maxSpanFromNext =
    nextLineStartMs && nextLineStartMs > lineStart
      ? Math.min(Math.max(550, nextLineStartMs - lineStart - 70), 6200)
      : Math.min(Math.max(1200, line.end_ms - lineStart), 5200);

  const weights = rawWords.map(charWeight);
  const totalWeight = weights.reduce((acc, v) => acc + v, 0) || rawWords.length;

  let cursor = lineStart;
  return rawWords.map((text, idx) => {
    const share = (weights[idx] / totalWeight) * maxSpanFromNext;
    const startMs = Math.round(cursor);
    cursor += share;
    const endMs = Math.max(startMs + 110, Math.round(cursor));
    return { text, startMs, endMs };
  });
}

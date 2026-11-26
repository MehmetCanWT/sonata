import {
  Check,
  Disc3,
  Globe,
  Heart,
  Languages,
  ListMusic,
  LocateFixed,
  MessageSquareQuote,
  Minimize2,
  Pause,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  Volume1,
  Volume2,
  VolumeX,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ArtworkImage } from "@/components/ArtworkImage";
import { Slider } from "@/components/ui/slider";
import { getLanguageLabel, getStrings } from "@/i18n";
import { windowToggleFullscreen } from "@/lib/ipc";
import {
  buildKaraokeTokens,
  computeTokenProgress,
  isBackgroundLine,
  isCounterSpeaker,
  isLyricLineActive,
  resolveActiveLineIndex,
} from "@/lib/lyricsSync";
import {
  cn,
  formatDuration,
  formatRemainingDuration,
  sanitizeAccentRgb,
  sanitizeAmbientBgRgb,
} from "@/lib/utils";
import { useInterpolatedPosition, useSonataStore } from "@/store/useSonataStore";
import type { TimedLine } from "@/types/sonata";

type DisplayRow =
  | { kind: "instrumental"; startMs: number; endMs: number }
  | { kind: "verse"; index: number; line: TimedLine; nextLineStartMs?: number };

const INSTRUMENTAL_GAP_MS = 5000;

function buildDisplayRows(lines: TimedLine[]): DisplayRow[] {
  const rows: DisplayRow[] = [];
  let runningMaxEnd = 0;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const nextLineStartMs = lines[i + 1]?.start_ms;
    if (i === 0 && line.start_ms >= INSTRUMENTAL_GAP_MS) {
      rows.push({ kind: "instrumental", startMs: 0, endMs: line.start_ms });
    } else if (i > 0 && line.start_ms - runningMaxEnd >= INSTRUMENTAL_GAP_MS) {
      rows.push({ kind: "instrumental", startMs: runningMaxEnd, endMs: line.start_ms });
    }
    const lineEnd = line.end_ms > line.start_ms ? line.end_ms : line.start_ms + 3000;
    if (lineEnd > runningMaxEnd) {
      runningMaxEnd = lineEnd;
    }
    if (line.text.trim()) {
      rows.push({ kind: "verse", index: i, line, nextLineStartMs });
    }
  }
  return rows;
}

function GlowingKaraokeVerse({
  line,
  nextLineStartMs,
  posMs,
  isCounter,
  isBg,
  reduceMotion,
}: {
  line: TimedLine;
  nextLineStartMs?: number;
  posMs: number;
  isCounter: boolean;
  isBg: boolean;
  reduceMotion: boolean;
}) {
  const tokens = useMemo(
    () => buildKaraokeTokens(line, nextLineStartMs),
    [line, nextLineStartMs],
  );
  const sizeClass = isBg
    ? "text-lg xl:text-[22px] font-semibold leading-[1.34] tracking-tight"
    : "text-2xl xl:text-[31px] font-bold leading-[1.34] tracking-tight";

  if (tokens.length === 0) {
    return (
      <p
        className={cn(
          sizeClass,
          "text-white",
          (isCounter || isBg) && "italic",
          isCounter ? "text-right" : "text-left",
        )}
      >
        {line.text}
      </p>
    );
  }

  return (
    <p
      className={cn(
        sizeClass,
        "flex flex-wrap gap-x-[0.28em]",
        (isCounter || isBg) && "italic",
        isCounter ? "justify-end" : "justify-start",
      )}
    >
      {tokens.map((tok, wi) => {
        const p = reduceMotion
          ? posMs >= tok.startMs
            ? 1
            : 0
          : computeTokenProgress(posMs, tok.startMs, tok.endMs);
        const pct = p * 100;
        const isSweeping = p > 0 && p < 1;
        const glowStrength = isSweeping ? 0.52 * Math.sin(p * Math.PI) + 0.18 : p === 1 ? 0.16 : 0;

        if (p <= 0) {
          return (
            <span key={`${tok.startMs}-${wi}`} className="inline-block text-white/32">
              {tok.text}
            </span>
          );
        }

        if (p >= 1) {
          return (
            <span
              key={`${tok.startMs}-${wi}`}
              className="inline-block text-white"
              style={{
                textShadow: reduceMotion ? undefined : "0 0 14px rgba(255,255,255,0.22)",
              }}
            >
              {tok.text}
            </span>
          );
        }

        const lead = Math.max(0, pct - 4).toFixed(1);
        const trail = Math.min(100, pct + 14).toFixed(1);
        return (
          <span
            key={`${tok.startMs}-${wi}`}
            className="inline-block"
            style={{
              backgroundImage: `linear-gradient(90deg, #ffffff 0%, #ffffff ${lead}%, rgba(255,255,255,0.34) ${trail}%, rgba(255,255,255,0.34) 100%)`,
              WebkitBackgroundClip: "text",
              backgroundClip: "text",
              color: "transparent",
              WebkitTextFillColor: "transparent",
              filter: `drop-shadow(0 0 10px rgba(255,255,255,${glowStrength.toFixed(2)}))`,
              transform: "translateY(-0.5px)",
            }}
          >
            {tok.text}
          </span>
        );
      })}
    </p>
  );
}

export function ImmersiveLyricsView() {
  const snapshot = useSonataStore((s) => s.snapshot);
  const tab = useSonataStore((s) => s.immersiveTab);
  const setTab = useSonataStore((s) => s.setImmersiveTab);
  const setImmersiveOpen = useSonataStore((s) => s.setImmersiveOpen);
  const dispatch = useSonataStore((s) => s.dispatch);
  const setVolumeOptimistic = useSonataStore((s) => s.setVolumeOptimistic);

  const posSecs = useInterpolatedPosition();
  const posMs = Math.round(posSecs * 1000);
  const activeLineRef = useRef<HTMLButtonElement | null>(null);
  const scrollerRef = useRef<HTMLDivElement | null>(null);
  const touchStartYRef = useRef<number | null>(null);
  const osFullscreenRef = useRef(false);

  const t = getStrings(snapshot.config.language);
  const targetLangLabel = getLanguageLabel(snapshot.config.language);
  const currentTrack =
    snapshot.current_index != null ? snapshot.queue[snapshot.current_index] : undefined;
  const durSecs = Math.max(snapshot.duration_secs || currentTrack?.duration_secs || 0, 0);
  const remainingSecs = Math.max(0, durSecs - posSecs);
  const liked = currentTrack
    ? currentTrack.liked || snapshot.liked_track_ids.includes(currentTrack.id)
    : false;

  const [ar, ag, ab] = sanitizeAmbientBgRgb(snapshot.ambient_rgb);
  const [br, bg, bb] = sanitizeAccentRgb(snapshot.ambient_rgb);
  const reduceMotion = snapshot.config.reduce_motion;

  const lines = useMemo(
    () => (snapshot.lyrics.kind === "Synced" ? snapshot.lyrics.lines : []),
    [snapshot.lyrics],
  );
  const displayRows = useMemo(() => buildDisplayRows(lines), [lines]);
  const hasDuets = useMemo(() => lines.some(isCounterSpeaker), [lines]);
  const activeIdx = useMemo(() => resolveActiveLineIndex(lines, posMs), [lines, posMs]);

  const [autoFollow, setAutoFollow] = useState(true);

  const scrollToActiveLine = useCallback(
    (instant = false) => {
      const target = activeLineRef.current;
      if (!target) return;
      target.scrollIntoView({
        behavior: instant || reduceMotion ? "auto" : "smooth",
        block: "center",
      });
    },
    [reduceMotion],
  );

  useEffect(() => {
    setAutoFollow(true);
  }, [currentTrack?.id, tab]);

  useEffect(() => {
    if (tab === "lyrics" && autoFollow && activeIdx >= 0) {
      scrollToActiveLine(false);
    }
  }, [activeIdx, autoFollow, scrollToActiveLine, tab]);

  useEffect(() => {
    if (tab !== "lyrics" || !autoFollow || activeIdx < 0) return;
    const scroller = scrollerRef.current;
    const recenter = () => {
      if (activeIdx >= 0) {
        scrollToActiveLine(true);
      }
    };
    const ro =
      scroller && typeof ResizeObserver !== "undefined" ? new ResizeObserver(recenter) : null;
    if (ro && scroller) {
      ro.observe(scroller);
    }
    window.addEventListener("resize", recenter);
    return () => {
      ro?.disconnect();
      window.removeEventListener("resize", recenter);
    };
  }, [activeIdx, autoFollow, scrollToActiveLine, tab]);

  const closeFullscreen = async () => {
    if (osFullscreenRef.current) {
      osFullscreenRef.current = false;
      await windowToggleFullscreen().catch(() => {});
    }
    setImmersiveOpen(false);
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        void closeFullscreen();
      } else if (e.key === "F11") {
        e.preventDefault();
        void windowToggleFullscreen().then((next) => {
          osFullscreenRef.current = next;
        });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const showSidePanel = tab !== "cover";

  return (
    <div
      onDoubleClick={() => {
        void windowToggleFullscreen().then((next) => {
          osFullscreenRef.current = next;
        });
      }}
      className="fixed inset-0 z-40 flex flex-col overflow-hidden bg-[#0c0a0e] select-none"
    >
      {/* Dual-hue ambient mesh backdrop matching classic GPUI Fullscreen */}
      <div
        className="pointer-events-none absolute inset-0"
        style={{
          background: `
            radial-gradient(ellipse 68% 72% at 24% 38%, rgba(${ar}, ${ag}, ${ab}, 0.42) 0%, transparent 72%),
            radial-gradient(ellipse 62% 68% at 76% 64%, rgba(${br}, ${bg}, ${bb}, 0.24) 0%, transparent 74%),
            radial-gradient(ellipse 100% 100% at 50% 50%, transparent 45%, rgba(6, 5, 8, 0.72) 100%)
          `,
        }}
      />

      {/* Main balanced 50/50 stage */}
      <div
        className="relative z-10 flex flex-1 min-h-0 flex-col md:flex-row items-center justify-center gap-6 md:gap-10 px-6 md:px-10 pt-8 pb-20"
        onDoubleClick={(e) => e.stopPropagation()}
      >
        {/* Left column: Cover art + minimal clean metadata + scrub + transport + volume */}
        <div
          className={cn(
            "flex min-w-0 flex-col items-center justify-center gap-5",
            showSidePanel ? "w-full md:flex-1" : "w-full max-w-[540px]",
          )}
        >
          <ArtworkImage
            src={snapshot.art_path || currentTrack?.thumbnail_url}
            motionSrc={
              snapshot.config.motion_covers && !snapshot.config.reduce_motion
                ? snapshot.motion_cover_path
                : null
            }
            motionPaused={snapshot.paused}
            alt={currentTrack?.title || "Sonata"}
            className={cn(
              "shrink-0 shadow-[0_20px_48px_rgba(0,0,0,0.56)] border border-white/10",
              showSidePanel
                ? "size-[clamp(190px,min(42vh,32vw),430px)]"
                : "size-[clamp(220px,min(50vh,40vw),500px)]",
            )}
            rounded="rounded-2xl"
          />

          <div className="flex w-full max-w-[430px] flex-col items-center gap-3">
            {currentTrack ? (
              <>
                <div className="flex w-full items-center justify-center gap-2">
                  {currentTrack.is_explicit && (
                    <span className="invisible shrink-0 px-1 text-[10px] font-bold">E</span>
                  )}
                  <h1 className="min-w-0 truncate text-center text-[22px] font-semibold tracking-tight text-white/95">
                    {currentTrack.title}
                  </h1>
                  {currentTrack.is_explicit && (
                    <span className="flex h-4 min-w-4 shrink-0 items-center justify-center rounded bg-white/15 px-1 text-[10px] font-bold text-white/65">
                      E
                    </span>
                  )}
                </div>

                <button
                  type="button"
                  onClick={() => {
                    void closeFullscreen();
                    void dispatch({ type: "OpenArtistByName", name: currentTrack.artist });
                  }}
                  className="-mt-1 max-w-full truncate text-sm font-medium text-white/65 hover:text-white transition-colors"
                >
                  {currentTrack.artist}
                </button>
              </>
            ) : (
              <p className="text-base text-white/50">{t.noTrackPlaying}</p>
            )}

            {/* Scrub bar + timestamps */}
            <div className="mt-1 flex w-full flex-col gap-1.5">
              <Slider
                aria-label={t.fullscreenPosition}
                value={[durSecs > 0 ? Math.min(posSecs, durSecs) : 0]}
                max={durSecs > 0 ? durSecs : 100}
                step={0.5}
                onValueChange={([val]) => {
                  if (durSecs > 0) {
                    void dispatch({ type: "SeekAbsolute", secs: val });
                  }
                }}
                trackClassName="h-[4px] group-hover:h-[6px] bg-white/15"
                rangeClassName="bg-white/90"
              />
              <div className="flex items-center justify-between text-[11px] font-medium text-white/50 tabular-nums">
                <span>{formatDuration(posSecs)}</span>
                <span>{formatRemainingDuration(remainingSecs)}</span>
              </div>
            </div>

            {/* Clean transport controls */}
            <div className="flex items-center justify-center gap-3">
              <button
                type="button"
                onClick={() => void dispatch({ type: "ToggleShuffle" })}
                aria-label={t.shuffle}
                className={cn(
                  "flex size-9 items-center justify-center rounded-full transition-colors hover:bg-white/10",
                  snapshot.shuffle ? "text-[var(--accent)]" : "text-white/65 hover:text-white",
                )}
              >
                <Shuffle className="size-4" />
              </button>

              <button
                type="button"
                disabled={!currentTrack}
                onClick={() => void dispatch({ type: "PrevTrack" })}
                aria-label={t.previousTrack}
                className="flex size-10 items-center justify-center rounded-full text-white/85 hover:bg-white/10 hover:text-white transition-colors disabled:opacity-35"
              >
                <SkipBack className="size-5 fill-current" />
              </button>

              <button
                type="button"
                disabled={!currentTrack && snapshot.queue.length === 0}
                onClick={() => void dispatch({ type: "TogglePause" })}
                aria-label={snapshot.paused ? t.play : t.pause}
                className="matte-btn flex size-12 items-center justify-center rounded-full bg-white text-[#111114] shadow-lg disabled:opacity-40"
              >
                {snapshot.paused ? (
                  <Play className="size-5 fill-current translate-x-[0.5px]" />
                ) : (
                  <Pause className="size-5 fill-current" />
                )}
              </button>

              <button
                type="button"
                disabled={!currentTrack}
                onClick={() => void dispatch({ type: "NextTrack" })}
                aria-label={t.nextTrack}
                className="flex size-10 items-center justify-center rounded-full text-white/85 hover:bg-white/10 hover:text-white transition-colors disabled:opacity-35"
              >
                <SkipForward className="size-5 fill-current" />
              </button>

              <button
                type="button"
                onClick={() => void dispatch({ type: "CycleRepeat" })}
                aria-label={t.cycleRepeat}
                className={cn(
                  "flex size-9 items-center justify-center rounded-full transition-colors hover:bg-white/10",
                  snapshot.repeat !== "Off"
                    ? "text-[var(--accent)]"
                    : "text-white/65 hover:text-white",
                )}
              >
                {snapshot.repeat === "One" ? (
                  <Repeat1 className="size-4" />
                ) : (
                  <Repeat className="size-4" />
                )}
              </button>

              {currentTrack && (
                <button
                  type="button"
                  onClick={() => void dispatch({ type: "ToggleLike", track: currentTrack })}
                  aria-label={t.likeTrack}
                  className={cn(
                    "flex size-9 items-center justify-center rounded-full transition-colors hover:bg-white/10",
                    liked ? "text-[var(--accent)]" : "text-white/65 hover:text-white",
                  )}
                >
                  {liked ? <Check className="size-4" /> : <Heart className="size-4" />}
                </button>
              )}
            </div>

            {/* Clean minimal volume slider only (no badge/speed/sleep clutter) */}
            <div className="flex w-40 items-center gap-2 pt-1">
              <button
                type="button"
                onClick={() => void dispatch({ type: "ToggleMute" })}
                aria-label={t.toggleMute}
                className="text-white/60 hover:text-white transition-colors"
              >
                {snapshot.muted || snapshot.volume === 0 ? (
                  <VolumeX className="size-3.5" />
                ) : snapshot.volume < 45 ? (
                  <Volume1 className="size-3.5" />
                ) : (
                  <Volume2 className="size-3.5" />
                )}
              </button>
              <Slider
                aria-label={t.fullscreenVolume}
                value={[snapshot.muted ? 0 : snapshot.volume]}
                max={100}
                step={1}
                onValueChange={([v]) => setVolumeOptimistic(v)}
                trackClassName="h-1 bg-white/15"
                rangeClassName="bg-white/80"
                className="flex-1"
              />
            </div>
          </div>
        </div>

        {/* Right column: Borderless depth-of-field letter-by-letter glowing Lyrics or Queue */}
        {showSidePanel && (
          <div className="flex-1 min-w-0 min-h-0 h-full w-full flex flex-col">
            {tab === "lyrics" ? (
              <div className="relative flex-1 min-h-0 h-full">
                {snapshot.lyrics.kind === "Synced" && !autoFollow && (
                  <div className="pointer-events-none absolute inset-x-0 top-2 z-20 flex justify-center">
                    <button
                      type="button"
                      onClick={() => {
                        setAutoFollow(true);
                        scrollToActiveLine(false);
                      }}
                      className="pointer-events-auto flex items-center gap-1.5 rounded-full border border-white/15 bg-black/70 px-3.5 py-1.5 text-xs font-semibold text-white/90 shadow-lg backdrop-blur-md hover:bg-white/15 hover:text-white transition-colors"
                    >
                      <LocateFixed className="size-3.5" />
                      <span>{t.syncLyrics}</span>
                    </button>
                  </div>
                )}

                <div
                  ref={scrollerRef}
                  onWheel={(e) => {
                    if (snapshot.lyrics.kind === "Synced" && Math.abs(e.deltaY) >= 4) {
                      setAutoFollow(false);
                    }
                  }}
                  onTouchStart={(e) => {
                    touchStartYRef.current = e.touches[0]?.clientY ?? null;
                  }}
                  onTouchMove={(e) => {
                    if (snapshot.lyrics.kind !== "Synced" || touchStartYRef.current === null) {
                      return;
                    }
                    const y = e.touches[0]?.clientY;
                    if (typeof y === "number" && Math.abs(y - touchStartYRef.current) >= 12) {
                      setAutoFollow(false);
                    }
                  }}
                  onTouchEnd={() => {
                    touchStartYRef.current = null;
                  }}
                  onTouchCancel={() => {
                    touchStartYRef.current = null;
                  }}
                  className="no-scrollbar h-full overflow-y-auto px-2 sm:px-6"
                  style={{
                    maskImage:
                      "linear-gradient(180deg, transparent 0%, black 14%, black 84%, transparent 100%)",
                    WebkitMaskImage:
                      "linear-gradient(180deg, transparent 0%, black 14%, black 84%, transparent 100%)",
                  }}
                >
                  {snapshot.lyrics.kind === "Loading" ? (
                    <div className="flex h-full items-center justify-center text-lg text-white/55">
                      {t.loadingLyrics}
                    </div>
                  ) : snapshot.lyrics.kind === "Synced" ? (
                    <div className="py-[28vh] flex flex-col gap-6">
                      {displayRows.map((row, rIdx) => {
                        if (row.kind === "instrumental") {
                          const activeBreak = posMs >= row.startMs && posMs < row.endMs;
                          return (
                            <div
                              key={`inst-${rIdx}`}
                              className={cn(
                                "flex items-center gap-2 py-2 text-lg tracking-[0.35em] transition-opacity",
                                activeBreak ? "text-white/75" : "text-white/20",
                              )}
                            >
                              <span>• • •</span>
                            </div>
                          );
                        }

                        const { index, line, nextLineStartMs } = row;
                        const isCounter = isCounterSpeaker(line);
                        const isBg = isBackgroundLine(line);
                        const isScrollAnchor = index === activeIdx;
                        const isActive = isLyricLineActive(lines, index, activeIdx, posMs);
                        const isPast = !isActive && activeIdx >= 0 && index < activeIdx;
                        const translated = snapshot.translated_lyrics?.[index];
                        const romanized = snapshot.romanized_lyrics?.[index];

                        return (
                          <button
                            key={`${line.start_ms}-${index}`}
                            ref={isScrollAnchor ? activeLineRef : undefined}
                            type="button"
                            onClick={() => {
                              setAutoFollow(true);
                              void dispatch({ type: "SeekAbsolute", secs: line.start_ms / 1000 });
                            }}
                            className={cn(
                              "block w-full rounded-xl px-3 py-1.5 transition-all duration-200 hover:bg-white/5",
                              isBg && "-mt-3",
                              isCounter ? "text-right" : "text-left",
                              hasDuets && (isCounter ? "pl-8 sm:pl-14" : "pr-8 sm:pr-14"),
                            )}
                          >
                            {isActive ? (
                              <GlowingKaraokeVerse
                                line={line}
                                nextLineStartMs={nextLineStartMs}
                                posMs={posMs}
                                isCounter={isCounter}
                                isBg={isBg}
                                reduceMotion={reduceMotion}
                              />
                            ) : (
                              <p
                                className={cn(
                                  isBg
                                    ? "text-lg xl:text-[22px] font-semibold leading-[1.34] tracking-tight transition-colors"
                                    : "text-2xl xl:text-[31px] font-bold leading-[1.34] tracking-tight transition-colors",
                                  (isCounter || isBg) && "italic",
                                  isPast
                                    ? "text-white/24"
                                    : index === activeIdx + 1
                                      ? "text-white/44"
                                      : "text-white/20",
                                )}
                              >
                                {line.text}
                              </p>
                            )}
                            {romanized && (
                              <p className="mt-1 text-sm text-white/45">{romanized}</p>
                            )}
                            {translated && (
                              <p className="mt-0.5 text-base font-medium text-white/62">
                                {translated}
                              </p>
                            )}
                          </button>
                        );
                      })}
                    </div>
                  ) : snapshot.lyrics.kind === "Plain" ? (
                    <div className="space-y-4 py-12">
                      {snapshot.lyrics.text.split("\n").map((line, idx) => {
                        const translated = snapshot.translated_lyrics?.[idx];
                        const romanized = snapshot.romanized_lyrics?.[idx];
                        if (!line.trim()) {
                          return <div key={`plain-spacer-${idx}`} className="h-3" />;
                        }
                        return (
                          <div key={`plain-${idx}`} className="space-y-0.5">
                            <p className="font-sans text-2xl font-semibold leading-relaxed text-white/85">
                              {line}
                            </p>
                            {romanized && (
                              <p className="text-sm text-white/45">{romanized}</p>
                            )}
                            {translated && (
                              <p className="text-base font-medium text-white/62">
                                {translated}
                              </p>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  ) : (
                    <div className="flex h-full items-center justify-center text-lg text-white/45">
                      {t.noLyrics}
                    </div>
                  )}
                </div>
              </div>
            ) : (
              <div
                className="no-scrollbar h-full overflow-y-auto px-2 sm:px-4 py-6"
                style={{
                  maskImage:
                    "linear-gradient(180deg, transparent 0%, black 6%, black 92%, transparent 100%)",
                  WebkitMaskImage:
                    "linear-gradient(180deg, transparent 0%, black 6%, black 92%, transparent 100%)",
                }}
              >
                <div className="mb-3 text-xs font-semibold uppercase tracking-wider text-white/55">
                  {t.playingNext}
                </div>
                {snapshot.queue.length === 0 ? (
                  <p className="py-10 text-sm text-white/45">{t.emptyQueue}</p>
                ) : (
                  <div className="flex flex-col gap-1">
                    {snapshot.queue.map((track, qIdx) => {
                      const isCur = qIdx === snapshot.current_index;
                      return (
                        <button
                          key={`${track.id}-${qIdx}`}
                          type="button"
                          onClick={() =>
                            void dispatch({ type: "PlayQueueIndex", index: qIdx })
                          }
                          className={cn(
                            "flex items-center gap-3 rounded-xl px-3 py-2 text-left transition-colors",
                            isCur ? "bg-white/12" : "hover:bg-white/6",
                          )}
                        >
                          <span
                            className={cn(
                              "w-5 text-center text-xs tabular-nums",
                              isCur ? "text-[var(--accent)] font-semibold" : "text-white/40",
                            )}
                          >
                            {qIdx + 1}
                          </span>
                          <ArtworkImage
                            src={track.thumbnail_url}
                            alt={track.title}
                            className="size-9 shrink-0"
                            rounded="rounded-md"
                          />
                          <div className="min-w-0 flex-1">
                            <div
                              className={cn(
                                "truncate text-sm font-medium",
                                isCur ? "text-[var(--accent)]" : "text-white/90",
                              )}
                            >
                              {track.title}
                            </div>
                            <div className="truncate text-xs text-white/55">{track.artist}</div>
                          </div>
                          <span className="text-xs text-white/45 tabular-nums">
                            {formatDuration(track.duration_secs)}
                          </span>
                        </button>
                      );
                    })}
                  </div>
                )}
              </div>
            )}
          </div>
        )}
      </div>

      {/* Ultra-minimal bottom-left corner pill */}
      <div
        className="absolute bottom-5 left-6 z-20 flex items-center gap-2"
        onDoubleClick={(e) => e.stopPropagation()}
      >
        <button
          type="button"
          onClick={() => void closeFullscreen()}
          aria-label={t.exitFullscreen}
          className="flex size-9 items-center justify-center rounded-xl border border-white/12 bg-black/40 text-white/85 hover:bg-white/14 hover:text-white transition-colors"
        >
          <Minimize2 className="size-4" />
        </button>

        <div className="flex items-center gap-0.5 rounded-xl border border-white/12 bg-black/40 p-1">
          <button
            type="button"
            onClick={() => setTab("cover")}
            title={t.cover}
            aria-label={t.coverMode}
            className={cn(
              "flex size-8 items-center justify-center rounded-lg transition-colors",
              tab === "cover"
                ? "bg-white/18 text-white"
                : "text-white/55 hover:bg-white/10 hover:text-white",
            )}
          >
            <Disc3 className="size-4" />
          </button>
          <button
            type="button"
            onClick={() => setTab("lyrics")}
            title={t.lyrics}
            aria-label={t.lyricsMode}
            className={cn(
              "flex size-8 items-center justify-center rounded-lg transition-colors",
              tab === "lyrics"
                ? "bg-white/18 text-white"
                : "text-white/55 hover:bg-white/10 hover:text-white",
            )}
          >
            <MessageSquareQuote className="size-4" />
          </button>
          <button
            type="button"
            onClick={() => setTab("queue")}
            title={t.playingNext}
            aria-label={t.queueMode}
            className={cn(
              "flex size-8 items-center justify-center rounded-lg transition-colors",
              tab === "queue"
                ? "bg-white/18 text-white"
                : "text-white/55 hover:bg-white/10 hover:text-white",
            )}
          >
            <ListMusic className="size-4" />
          </button>
        </div>

        <div className="flex items-center gap-0.5 rounded-xl border border-white/12 bg-black/40 p-1">
          <button
            type="button"
            onClick={() => void dispatch({ type: "ToggleLyricsRomanize" })}
            title={t.romanize}
            aria-label={t.romanize}
            aria-pressed={snapshot.config.lyrics_romanize}
            className={cn(
              "flex size-8 items-center justify-center rounded-lg transition-colors",
              snapshot.config.lyrics_romanize
                ? "bg-white/18 text-white"
                : "text-white/55 hover:bg-white/10 hover:text-white",
            )}
          >
            <Globe className="size-4" />
          </button>
          <button
            type="button"
            onClick={() => void dispatch({ type: "ToggleLyricsTranslate" })}
            title={`${t.translate} (${targetLangLabel})`}
            aria-label={`${t.translate} (${targetLangLabel})`}
            aria-pressed={snapshot.config.lyrics_translate}
            className={cn(
              "flex size-8 items-center justify-center rounded-lg transition-colors",
              snapshot.config.lyrics_translate
                ? "bg-white/18 text-white"
                : "text-white/55 hover:bg-white/10 hover:text-white",
            )}
          >
            <Languages className="size-4" />
          </button>
        </div>
      </div>
    </div>
  );
}

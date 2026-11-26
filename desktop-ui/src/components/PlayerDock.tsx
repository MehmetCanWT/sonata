import {
  Check,
  Heart,
  ListMusic,
  Mic2,
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
import { ArtworkImage } from "@/components/ArtworkImage";
import { Slider } from "@/components/ui/slider";
import { getStrings } from "@/i18n";
import {
  cn,
  formatDuration,
  formatRemainingDuration,
} from "@/lib/utils";
import { useInterpolatedPosition, useSonataStore } from "@/store/useSonataStore";

interface PlayerDockProps {
  compact?: boolean;
}

export function PlayerDock({ compact = false }: PlayerDockProps) {
  const snapshot = useSonataStore((s) => s.snapshot);
  const queueOpen = useSonataStore((s) => s.queueOpen);
  const immersiveOpen = useSonataStore((s) => s.immersiveOpen);
  const toggleQueue = useSonataStore((s) => s.toggleQueue);
  const openImmersive = useSonataStore((s) => s.openImmersive);
  const dispatch = useSonataStore((s) => s.dispatch);
  const setVolumeOptimistic = useSonataStore((s) => s.setVolumeOptimistic);
  const t = getStrings(snapshot.config.language);

  const posSecs = useInterpolatedPosition();
  const currentTrack =
    snapshot.current_index != null ? snapshot.queue[snapshot.current_index] : undefined;
  const durSecs = Math.max(snapshot.duration_secs || currentTrack?.duration_secs || 0, 0);
  const remainingSecs = Math.max(0, durSecs - posSecs);
  const liked = currentTrack
    ? currentTrack.liked || snapshot.liked_track_ids.includes(currentTrack.id)
    : false;

  const iconBtn =
    "flex size-8 shrink-0 items-center justify-center rounded-full text-[var(--ink-secondary)] hover:text-[var(--ink-primary)] hover:bg-[var(--bg-hover)] transition-colors disabled:opacity-35";

  return (
    <div
      className={cn(
        "@container relative z-20 rounded-2xl border border-[var(--border-strong)] bg-[var(--bg-dock)] px-5 pt-3 pb-3.5",
        compact ? "mx-auto w-[min(740px,calc(100%-2rem))]" : "mx-3 mb-3 w-[calc(100%-1.5rem)]",
      )}
      style={{ boxShadow: "var(--dock-shadow)" }}
    >
      {/* Top interactive scrub track + timestamps */}
      <div className="mb-2.5 flex items-center gap-3">
        <span className="w-10 shrink-0 text-[11px] font-medium text-[var(--ink-muted)] tabular-nums">
          {formatDuration(posSecs)}
        </span>
        <Slider
          aria-label={t.playbackPosition}
          value={[durSecs > 0 ? Math.min(posSecs, durSecs) : 0]}
          max={durSecs > 0 ? durSecs : 100}
          step={0.5}
          onValueChange={([val]) => {
            if (durSecs > 0) {
              void dispatch({ type: "SeekAbsolute", secs: val });
            }
          }}
          trackClassName="h-[4px] group-hover:h-[6px] bg-[var(--border-strong)]"
          rangeClassName="bg-[var(--ink-primary)]"
          className="flex-1"
        />
        <span className="w-11 shrink-0 text-right text-[11px] font-medium text-[var(--ink-muted)] tabular-nums">
          {formatRemainingDuration(remainingSecs)}
        </span>
      </div>

      {/* Bottom 3-column dock: clean transport left, recessed now-playing capsule center, mic/queue/fullscreen right */}
      <div className="flex items-center justify-between gap-3 @[680px]:grid @[680px]:grid-cols-[1fr_minmax(260px,1.45fr)_1fr] @[680px]:gap-4">
        {/* Left: clean transport controls (no noisy bitrate badge text) */}
        <div className="flex shrink-0 items-center gap-1.5 min-w-0">
          <button
            type="button"
            onClick={() => void dispatch({ type: "ToggleShuffle" })}
            aria-label={t.shuffle}
            className={cn(iconBtn, "hidden @[520px]:flex", snapshot.shuffle && "text-[var(--accent)]")}
          >
            <Shuffle className="size-4" />
          </button>
          <button
            type="button"
            onClick={() => void dispatch({ type: "PrevTrack" })}
            disabled={!currentTrack}
            aria-label={t.previousTrack}
            className={iconBtn}
          >
            <SkipBack className="size-[18px] fill-current" />
          </button>
          <button
            type="button"
            onClick={() => void dispatch({ type: "TogglePause" })}
            disabled={!currentTrack && snapshot.queue.length === 0}
            aria-label={snapshot.paused ? t.play : t.pause}
            className="matte-btn flex size-10 shrink-0 items-center justify-center rounded-full bg-[var(--ink-primary)] text-[var(--bg-surface)] shadow-sm disabled:opacity-40"
          >
            {snapshot.paused ? (
              <Play className="size-[18px] fill-current translate-x-[0.5px]" />
            ) : (
              <Pause className="size-[18px] fill-current" />
            )}
          </button>
          <button
            type="button"
            onClick={() => void dispatch({ type: "NextTrack" })}
            disabled={!currentTrack}
            aria-label={t.nextTrack}
            className={iconBtn}
          >
            <SkipForward className="size-[18px] fill-current" />
          </button>
          <button
            type="button"
            onClick={() => void dispatch({ type: "CycleRepeat" })}
            aria-label={t.cycleRepeat}
            className={cn(
              iconBtn,
              "hidden @[520px]:flex",
              snapshot.repeat !== "Off" && "text-[var(--accent)]",
            )}
          >
            {snapshot.repeat === "One" ? (
              <Repeat1 className="size-4" />
            ) : (
              <Repeat className="size-4" />
            )}
          </button>
        </div>

        {/* Center: Larger recessed Now Playing capsule + clean inline like toggle */}
        <div className="flex flex-1 min-w-0 max-w-[460px] @[680px]:max-w-none items-center gap-2 rounded-xl border border-[var(--border-hairline)] bg-[var(--bg-capsule)] px-3 py-1.5 hover:border-[var(--border-strong)] transition-colors">
          <button
            type="button"
            onClick={() => {
              if (currentTrack?.album_id) {
                void dispatch({
                  type: "OpenAlbum",
                  album_id: currentTrack.album_id,
                  title: currentTrack.album,
                  artist: currentTrack.artist,
                  thumbnail_url: currentTrack.thumbnail_url,
                });
              } else if (currentTrack) {
                openImmersive("lyrics");
              }
            }}
            className="flex flex-1 min-w-0 items-center gap-3 text-left"
          >
            <ArtworkImage
              src={snapshot.art_path || currentTrack?.thumbnail_url}
              alt={currentTrack?.title || t.noTrackPlaying}
              className="size-10 shrink-0"
              rounded="rounded-lg"
            />
            <div className="min-w-0 flex-1">
              <div className="truncate text-[13px] font-semibold text-[var(--ink-primary)]">
                {snapshot.loading_track ? t.loadingStream : currentTrack?.title || "Sonata"}
              </div>
              <div className="truncate text-xs text-[var(--ink-secondary)]">
                {currentTrack
                  ? currentTrack.album
                    ? `${currentTrack.artist} · ${currentTrack.album}`
                    : currentTrack.artist
                  : t.selectTrackToPlay}
              </div>
            </div>
          </button>

          {currentTrack && (
            <button
              type="button"
              onClick={() => void dispatch({ type: "ToggleLike", track: currentTrack })}
              aria-label={t.likeTrack}
              className={cn(
                "flex size-7 shrink-0 items-center justify-center rounded-full text-[var(--ink-muted)] hover:bg-[var(--bg-hover)] hover:text-[var(--ink-primary)] transition-colors",
                liked && "text-[var(--accent)]",
              )}
            >
              {liked ? <Check className="size-3.5" /> : <Heart className="size-3.5" />}
            </button>
          )}
        </div>

        {/* Right: volume + microphone lyrics + queue */}
        <div className="flex shrink-0 items-center justify-end gap-1.5">
          <div className="hidden @[580px]:flex items-center gap-1.5 w-24 mr-1">
            <button
              type="button"
              onClick={() => void dispatch({ type: "ToggleMute" })}
              aria-label={t.toggleMute}
              className={iconBtn}
            >
              {snapshot.muted || snapshot.volume === 0 ? (
                <VolumeX className="size-4" />
              ) : snapshot.volume < 45 ? (
                <Volume1 className="size-4" />
              ) : (
                <Volume2 className="size-4" />
              )}
            </button>
            <Slider
              aria-label={t.volume}
              value={[snapshot.muted ? 0 : snapshot.volume]}
              max={100}
              step={1}
              onValueChange={([v]) => setVolumeOptimistic(v)}
              trackClassName="h-1 bg-[var(--border-strong)]"
              className="flex-1"
            />
          </div>

          <button
            type="button"
            onClick={() => openImmersive("lyrics")}
            aria-label={t.lyrics}
            title={t.lyrics}
            className={cn(iconBtn, immersiveOpen && "text-[var(--accent)]")}
          >
            <Mic2 className="size-4" />
          </button>

          <button
            type="button"
            onClick={toggleQueue}
            aria-label={t.queueMode}
            title={t.playbackQueue}
            className={cn(iconBtn, queueOpen && "text-[var(--accent)]")}
          >
            <ListMusic className="size-4" />
          </button>
        </div>
      </div>
    </div>
  );
}

import {
  ArrowDown,
  ArrowUp,
  Clock,
  Infinity as InfinityIcon,
  ListX,
  Repeat,
  Repeat1,
  Shuffle,
  X,
} from "lucide-react";
import { ArtworkImage } from "@/components/ArtworkImage";
import { getStrings } from "@/lib/i18n";
import { cn, formatDuration, formatQueueEta } from "@/lib/utils";
import { useInterpolatedPosition, useSonataStore } from "@/store/useSonataStore";

function QueueEtaPill({
  durationSecs,
  upcomingSecs,
  onOpenSleepTimer,
}: {
  durationSecs: number;
  upcomingSecs: number;
  onOpenSleepTimer: () => void;
}) {
  const posSecs = useInterpolatedPosition({ intervalMs: 5000 });
  const currentRemaining = Math.max(0, durationSecs - posSecs);
  const eta = formatQueueEta(currentRemaining + upcomingSecs);

  return (
    <button
      type="button"
      onClick={onOpenSleepTimer}
      className="absolute bottom-3 left-1/2 -translate-x-1/2 flex items-center gap-2 rounded-full border border-[var(--border-strong)] bg-[var(--bg-dock)] px-3.5 py-1.5 text-[11px] font-medium text-[var(--ink-secondary)] shadow-xl hover:text-[var(--ink-primary)] transition-colors tabular-nums"
    >
      <Clock className="h-3.5 w-3.5 text-[var(--ink-muted)]" />
      <span>
        {eta.durationLabel} → {eta.finishTimeLabel}
      </span>
    </button>
  );
}

export function QueuePanel() {
  const snapshot = useSonataStore((s) => s.snapshot);
  const dispatch = useSonataStore((s) => s.dispatch);
  const setModal = useSonataStore((s) => s.setModal);

  const t = getStrings(snapshot.config.language);
  const currentIdx = snapshot.current_index ?? 0;
  const currentTrack =
    snapshot.current_index != null ? snapshot.queue[snapshot.current_index] : undefined;
  const upcomingEntries = snapshot.queue
    .map((track, idx) => ({ track, idx }))
    .filter(({ idx }) => snapshot.current_index == null || idx > currentIdx);

  const upcomingSecs = upcomingEntries.reduce(
    (sum, { track }) => sum + (track.duration_secs ?? 180),
    0,
  );

  return (
    <aside
      aria-label={t.playbackQueue}
      className="relative flex h-full w-[288px] xl:w-[312px] shrink-0 flex-col bg-[var(--bg-sidebar)] pl-2.5 pr-1 select-none"
    >
      {/* Top Queue toolbar matching reference screenshot */}
      <div className="flex items-center justify-between gap-2 pb-3 pt-1">
        <div className="flex items-center gap-1 rounded-full border border-[var(--border-hairline)] bg-[var(--bg-elevated)] p-1">
          <button
            type="button"
            onClick={() => void dispatch({ type: "ToggleShuffle" })}
            aria-label={t.toggleShuffle}
            className={cn(
              "flex h-6 w-6 items-center justify-center rounded-full transition-colors",
              snapshot.shuffle
                ? "bg-[var(--bg-hover)] text-[var(--accent)]"
                : "text-[var(--ink-secondary)] hover:text-[var(--ink-primary)]",
            )}
          >
            <Shuffle className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            onClick={() => void dispatch({ type: "CycleRepeat" })}
            aria-label={t.cycleRepeat}
            className={cn(
              "flex h-6 w-6 items-center justify-center rounded-full transition-colors",
              snapshot.repeat !== "Off"
                ? "bg-[var(--bg-hover)] text-[var(--accent)]"
                : "text-[var(--ink-secondary)] hover:text-[var(--ink-primary)]",
            )}
          >
            {snapshot.repeat === "One" ? (
              <Repeat1 className="h-3.5 w-3.5" />
            ) : (
              <Repeat className="h-3.5 w-3.5" />
            )}
          </button>
          <button
            type="button"
            onClick={() => void dispatch({ type: "ToggleAutoplay" })}
            aria-label={t.toggleAutoplay}
            className={cn(
              "flex h-6 w-6 items-center justify-center rounded-full transition-colors",
              snapshot.autoplay
                ? "bg-[var(--bg-hover)] text-[var(--accent)]"
                : "text-[var(--ink-secondary)] hover:text-[var(--ink-primary)]",
            )}
          >
            <InfinityIcon className="h-3.5 w-3.5" />
          </button>
        </div>

        <div className="flex items-center gap-1.5">
          <span className="rounded-full border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-2.5 py-1 text-[11px] font-medium text-[var(--ink-secondary)] tabular-nums">
            {upcomingEntries.length} {t.items}
          </span>
          {snapshot.queue.length > 0 && (
            <button
              type="button"
              onClick={() => void dispatch({ type: "ClearQueue" })}
              aria-label={t.clearQueue}
              className="flex h-7 w-7 items-center justify-center rounded-full border border-[var(--border-hairline)] bg-[var(--bg-elevated)] text-[var(--ink-muted)] hover:text-[var(--ink-primary)]"
            >
              <ListX className="h-3.5 w-3.5" />
            </button>
          )}
        </div>
      </div>

      {/* Scrollable queue body */}
      <div className="sonata-scroll flex-1 overflow-y-auto pb-16 space-y-4">
        {currentTrack && (
          <div>
            <div className="mb-1.5 flex items-center justify-between px-1">
              <span className="text-xs font-semibold text-[var(--ink-primary)]">
                {t.nowPlaying}
              </span>
              <span className="text-[11px] text-[var(--ink-muted)] tabular-nums">
                {formatDuration(currentTrack.duration_secs)}
              </span>
            </div>
            <div className="flex items-center gap-2.5 rounded-xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] p-2">
              <ArtworkImage
                src={snapshot.art_path || currentTrack.thumbnail_url}
                alt={currentTrack.title}
                className="h-9 w-9"
                rounded="rounded-md"
              />
              <div className="min-w-0 flex-1">
                <div className="truncate text-xs font-semibold text-[var(--ink-primary)]">
                  {currentTrack.title}
                </div>
                <div className="truncate text-[11px] text-[var(--ink-secondary)]">
                  {currentTrack.artist}
                  {currentTrack.album ? ` — ${currentTrack.album}` : ""}
                </div>
              </div>
            </div>
          </div>
        )}

        <div>
          <div className="mb-1.5 flex items-center justify-between px-1">
            <div>
              <div className="text-xs font-semibold text-[var(--ink-primary)]">
                {t.playingNext}
              </div>
              {currentTrack?.album && (
                <div className="text-[11px] text-[var(--ink-muted)] truncate max-w-[190px]">
                  {t.fromPrefix} {currentTrack.album}
                </div>
              )}
            </div>
            {snapshot.queue.length > 0 && (
              <span className="text-[11px] text-[var(--ink-muted)] tabular-nums">
                {(snapshot.current_index ?? 0) + 1} {t.ofSeparator} {snapshot.queue.length}
              </span>
            )}
          </div>

          {upcomingEntries.length === 0 ? (
            <div className="rounded-xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)]/60 p-4 text-center text-xs text-[var(--ink-muted)]">
              {snapshot.autoplay ? t.autoplayHint : t.emptyQueue}
            </div>
          ) : (
            <div className="space-y-0.5">
              {upcomingEntries.map(({ track, idx }) => (
                <div
                  key={`${track.id}-${idx}`}
                  className="group flex items-center gap-2 rounded-lg px-1.5 py-1.5 hover:bg-[var(--bg-elevated)] transition-colors"
                >
                  <span className="w-5 text-center text-[11px] text-[var(--ink-muted)] tabular-nums">
                    {idx + 1}
                  </span>
                  <button
                    type="button"
                    onClick={() => void dispatch({ type: "PlayQueueIndex", index: idx })}
                    className="flex min-w-0 flex-1 items-center gap-2 text-left"
                  >
                    <ArtworkImage
                      src={track.thumbnail_url}
                      alt={track.title}
                      className="h-8 w-8"
                      rounded="rounded"
                    />
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-xs font-medium text-[var(--ink-primary)]">
                        {track.title}
                      </div>
                      <div className="truncate text-[11px] text-[var(--ink-secondary)]">
                        {track.artist}
                      </div>
                    </div>
                  </button>
                  <div className="hidden group-hover:flex items-center gap-0.5">
                    {idx > currentIdx + 1 && (
                      <button
                        type="button"
                        onClick={() =>
                          void dispatch({ type: "MoveQueueItem", from: idx, to: idx - 1 })
                        }
                        aria-label={t.moveUp}
                        className="rounded p-1 text-[var(--ink-muted)] hover:bg-[var(--bg-hover)] hover:text-[var(--ink-primary)]"
                      >
                        <ArrowUp className="h-3 w-3" />
                      </button>
                    )}
                    {idx + 1 < snapshot.queue.length && (
                      <button
                        type="button"
                        onClick={() =>
                          void dispatch({ type: "MoveQueueItem", from: idx, to: idx + 1 })
                        }
                        aria-label={t.moveDown}
                        className="rounded p-1 text-[var(--ink-muted)] hover:bg-[var(--bg-hover)] hover:text-[var(--ink-primary)]"
                      >
                        <ArrowDown className="h-3 w-3" />
                      </button>
                    )}
                    <button
                      type="button"
                      onClick={() => void dispatch({ type: "RemoveFromQueue", index: idx })}
                      aria-label={t.removeFromQueue}
                      className="rounded p-1 text-[var(--ink-muted)] hover:bg-[var(--bg-hover)] hover:text-red-400"
                    >
                      <X className="h-3 w-3" />
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="px-1 pt-1">
          <div className="flex items-center gap-1.5 text-xs font-semibold text-[var(--ink-primary)]">
            <InfinityIcon className="h-3.5 w-3.5 text-[var(--ink-secondary)]" />
            <span>{t.autoplay}</span>
          </div>
          <p className="mt-0.5 text-[11px] text-[var(--ink-muted)]">
            {snapshot.autoplay ? t.autoplayHint : t.autoplayPaused}
          </p>
        </div>
      </div>

      {/* Bottom floating ETA & Sleep Timer pill matching reference */}
      {snapshot.queue.length > 0 && (
        <QueueEtaPill
          durationSecs={snapshot.duration_secs || currentTrack?.duration_secs || 0}
          upcomingSecs={upcomingSecs}
          onOpenSleepTimer={() => setModal({ type: "sleepTimer" })}
        />
      )}
    </aside>
  );
}

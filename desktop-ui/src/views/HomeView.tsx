import { ChevronRight, Play, RefreshCw } from "lucide-react";
import { ArtworkImage } from "@/components/ArtworkImage";
import { HorizontalScroller } from "@/components/HorizontalScroller";
import { MediaCard } from "@/components/MediaCard";
import { Button } from "@/components/ui/button";
import { getStrings } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { useSonataStore } from "@/store/useSonataStore";

export function HomeView() {
  const snapshot = useSonataStore((s) => s.snapshot);
  const dispatch = useSonataStore((s) => s.dispatch);
  const t = getStrings(snapshot.config.language);

  if (snapshot.config.personal_only_mode) {
    return (
      <div className="sonata-scroll h-full overflow-y-auto px-5 @md:px-8 pt-5 pb-32 flex flex-col gap-6">
        <div className="flex shrink-0 items-center justify-between">
          <h1 className="text-3xl font-bold tracking-tight">{t.personalOnlyMode}</h1>
        </div>
        <p className="text-sm text-[var(--ink-secondary)]">
          {t.personalOnlyHomeDesc}
        </p>
      </div>
    );
  }

  const visibleShelves = snapshot.home_shelves.filter(
    (shelf) => !(shelf.cards.length > 0 && shelf.cards.every((c) => c.kind === "Mood")),
  );

  return (
    <div className="sonata-scroll h-full overflow-y-auto px-5 @md:px-8 pt-5 pb-36 flex flex-col gap-7">
      <div className="flex shrink-0 items-center justify-between">
        <h1 className="text-3xl font-bold tracking-tight">{t.home}</h1>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => void dispatch({ type: "RefreshHome" })}
        >
          <RefreshCw className={cn("h-3.5 w-3.5", snapshot.home_loading && "animate-spin")} />
          {t.refresh}
        </Button>
      </div>

      {/* Mood chips strip — zero scrollbar */}
      {snapshot.home_chips.length > 0 && (
        <HorizontalScroller
          scrollStep={280}
          contentClassName="gap-2 py-0.5"
        >
          {snapshot.home_chips.map((chip) => {
            const active = snapshot.active_home_chip?.params === chip.params;
            return (
              <button
                key={chip.params}
                type="button"
                onClick={() =>
                  void dispatch({
                    type: "SelectHomeChip",
                    chip: active ? null : chip,
                  })
                }
                className={cn(
                  "matte-btn shrink-0 rounded-full border px-3.5 py-1.5 text-xs font-medium",
                  active
                    ? "border-transparent bg-[var(--ink-primary)] text-[var(--bg-surface)]"
                    : "border-[var(--border-hairline)] bg-[var(--bg-elevated)] text-[var(--ink-secondary)] hover:text-[var(--ink-primary)]",
                )}
              >
                {chip.title}
              </button>
            );
          })}
        </HorizontalScroller>
      )}

      {snapshot.home_error && (
        <div className="shrink-0 rounded-xl border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-300">
          {snapshot.home_error}
        </div>
      )}

      {snapshot.home_loading && visibleShelves.length === 0 ? (
        <div className="py-12 text-sm text-[var(--ink-muted)]">{t.loadingHomeShelves}</div>
      ) : (
        visibleShelves.map((shelf) => (
          <section key={shelf.title} className="shrink-0 flex flex-col gap-3">
            <div className="flex items-center justify-between">
              <button
                type="button"
                onClick={() => void dispatch({ type: "OpenShelfDetail", shelf })}
                className="group inline-flex items-center gap-1 text-lg font-semibold tracking-tight text-[var(--ink-primary)]"
              >
                <span>{shelf.title}</span>
                <ChevronRight className="h-4 w-4 text-[var(--ink-muted)] transition-transform group-hover:translate-x-0.5" />
              </button>
            </div>

            {shelf.tracks.length > 0 ? (
              <div className="grid grid-cols-1 @lg:grid-cols-2 @2xl:grid-cols-3 gap-2">
                {shelf.tracks.slice(0, 12).map((track, idx) => (
                  <button
                    key={`${track.id}-${idx}`}
                    type="button"
                    onClick={() =>
                      void dispatch({
                        type: "PlayTracks",
                        tracks: shelf.tracks,
                        start_index: idx,
                        shuffle: false,
                      })
                    }
                    className="group flex items-center gap-3 rounded-xl border border-transparent bg-[var(--bg-elevated)]/40 p-2 text-left hover:border-[var(--border-hairline)] hover:bg-[var(--bg-elevated)] transition-colors"
                  >
                    <div className="relative h-11 w-11 shrink-0">
                      <ArtworkImage
                        src={track.thumbnail_url}
                        alt={track.title}
                        className="h-11 w-11"
                        rounded="rounded-lg"
                      />
                      <span className="absolute inset-0 flex items-center justify-center rounded-lg bg-black/50 opacity-0 group-hover:opacity-100 transition-opacity">
                        <Play className="h-4 w-4 fill-current text-white" />
                      </span>
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[13px] font-medium text-[var(--ink-primary)]">
                        {track.title}
                      </div>
                      <div className="truncate text-xs text-[var(--ink-secondary)]">
                        {track.artist}
                      </div>
                    </div>
                  </button>
                ))}
              </div>
            ) : (
              <HorizontalScroller contentClassName="gap-3 py-1">
                {shelf.cards.map((card) => (
                  <MediaCard
                    key={`${shelf.title}-${card.id}`}
                    card={card}
                    shelfCards={shelf.cards}
                  />
                ))}
              </HorizontalScroller>
            )}
          </section>
        ))
      )}
    </div>
  );
}

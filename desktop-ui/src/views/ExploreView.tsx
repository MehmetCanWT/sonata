import { ChevronRight, RefreshCw } from "lucide-react";
import { HorizontalScroller } from "@/components/HorizontalScroller";
import { MediaCard } from "@/components/MediaCard";
import { TrackRow } from "@/components/TrackRow";
import { Button } from "@/components/ui/button";
import { getStrings } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { useSonataStore } from "@/store/useSonataStore";

interface ExploreViewProps {
  mode: "explore" | "moodCategory" | "shelfDetail";
}

export function ExploreView({ mode }: ExploreViewProps) {
  const snapshot = useSonataStore((s) => s.snapshot);
  const dispatch = useSonataStore((s) => s.dispatch);
  const t = getStrings(snapshot.config.language);

  if (mode === "moodCategory") {
    return (
      <div className="sonata-scroll h-full overflow-y-auto px-5 @md:px-8 pt-5 pb-36 flex flex-col gap-7">
        <h1 className="text-3xl font-bold tracking-tight">
          {snapshot.mood_page_title || t.moodAndGenre}
        </h1>
        {snapshot.mood_page_loading && snapshot.mood_page_shelves.length === 0 ? (
          <div className="text-sm text-[var(--ink-muted)]">{t.loading}</div>
        ) : (
          snapshot.mood_page_shelves.map((shelf) => (
            <section key={shelf.title} className="space-y-3">
              <h2 className="text-lg font-semibold text-[var(--ink-primary)]">{shelf.title}</h2>
              {shelf.tracks.length > 0 ? (
                <div>
                  {shelf.tracks.map((tr, idx) => (
                    <TrackRow
                      key={`${tr.id}-${idx}`}
                      track={tr}
                      index={idx}
                      allTracks={shelf.tracks}
                      showCover
                      showAlbum
                    />
                  ))}
                </div>
              ) : (
                <HorizontalScroller contentClassName="gap-3 py-1">
                  {shelf.cards.map((card) => (
                    <MediaCard key={card.id} card={card} shelfCards={shelf.cards} />
                  ))}
                </HorizontalScroller>
              )}
            </section>
          ))
        )}
      </div>
    );
  }

  if (mode === "shelfDetail" && snapshot.shelf_detail) {
    const shelf = snapshot.shelf_detail;
    return (
      <div className="sonata-scroll h-full overflow-y-auto px-5 @md:px-8 pt-5 pb-36 flex flex-col gap-6">
        <h1 className="text-3xl font-bold tracking-tight">{shelf.title}</h1>
        {shelf.tracks.length > 0 ? (
          <div>
            {shelf.tracks.map((tr, idx) => (
              <TrackRow
                key={`${tr.id}-${idx}`}
                track={tr}
                index={idx}
                allTracks={shelf.tracks}
                showCover
                showAlbum
              />
            ))}
          </div>
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(156px,1fr))] gap-3">
            {shelf.cards.map((card) => (
              <MediaCard key={card.id} card={card} layout="grid" />
            ))}
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="sonata-scroll h-full overflow-y-auto px-5 @md:px-8 pt-5 pb-32 flex flex-col gap-7">
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold tracking-tight">{t.explore}</h1>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => void dispatch({ type: "RefreshExplore" })}
        >
          <RefreshCw className={cn("h-3.5 w-3.5", snapshot.explore_loading && "animate-spin")} />
          {t.refresh}
        </Button>
      </div>

      {snapshot.mood_tiles.length > 0 && (
        <section className="space-y-3">
          <h2 className="text-lg font-semibold">{t.moodsAndGenres}</h2>
          <div className="grid grid-cols-2 @md:grid-cols-3 @xl:grid-cols-4 gap-2.5">
            {snapshot.mood_tiles.map((tile) => (
              <button
                key={`${tile.browse_id}-${tile.params}`}
                type="button"
                onClick={() =>
                  void dispatch({
                    type: "OpenMoodCategory",
                    title: tile.title,
                    browse_id: tile.browse_id,
                    params: tile.params,
                  })
                }
                className="matte-card matte-card-interactive flex h-12 items-center justify-between rounded-xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-3.5 text-left"
              >
                <span className="truncate text-xs font-semibold text-[var(--ink-primary)]">
                  {tile.title}
                </span>
                <ChevronRight className="h-3.5 w-3.5 shrink-0 text-[var(--ink-muted)]" />
              </button>
            ))}
          </div>
        </section>
      )}

      {snapshot.explore_shelves.map((shelf) => (
        <section key={shelf.title} className="space-y-3">
          <button
            type="button"
            onClick={() => void dispatch({ type: "OpenShelfDetail", shelf })}
            className="inline-flex items-center gap-1 text-lg font-semibold text-[var(--ink-primary)]"
          >
            <span>{shelf.title}</span>
            <ChevronRight className="h-4 w-4 text-[var(--ink-muted)]" />
          </button>
          {shelf.tracks.length > 0 ? (
            <div>
              {shelf.tracks.slice(0, 10).map((tr, idx) => (
                <TrackRow
                  key={`${tr.id}-${idx}`}
                  track={tr}
                  index={idx}
                  allTracks={shelf.tracks}
                  showCover
                />
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
      ))}
    </div>
  );
}

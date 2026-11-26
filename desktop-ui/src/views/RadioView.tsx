import { Radio, RefreshCw } from "lucide-react";
import { HorizontalScroller } from "@/components/HorizontalScroller";
import { MediaCard } from "@/components/MediaCard";
import { TrackRow } from "@/components/TrackRow";
import { Button } from "@/components/ui/button";
import { getStrings } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { useSonataStore } from "@/store/useSonataStore";

export function RadioView() {
  const snapshot = useSonataStore((s) => s.snapshot);
  const dispatch = useSonataStore((s) => s.dispatch);
  const t = getStrings(snapshot.config.language);

  return (
    <div className="sonata-scroll h-full overflow-y-auto px-5 @md:px-8 pt-5 pb-36 flex flex-col gap-7">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">{t.radio}</h1>
          <p className="mt-1 text-xs text-[var(--ink-secondary)]">
            {t.radioDesc}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button
            size="pill"
            variant="accent"
            onClick={() => void dispatch({ type: "StartQuickRadio" })}
          >
            <Radio className="h-4 w-4" />
            {t.quickMix}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => void dispatch({ type: "RefreshRadio" })}
          >
            <RefreshCw className={cn("h-3.5 w-3.5", snapshot.radio_loading && "animate-spin")} />
            {t.refresh}
          </Button>
        </div>
      </div>

      {snapshot.radio_loading && snapshot.radio_shelves.length === 0 ? (
        <div className="text-sm text-[var(--ink-muted)]">{t.loadingRadioMixes}</div>
      ) : (
        snapshot.radio_shelves.map((shelf) => (
          <section key={shelf.title} className="space-y-3">
            <h2 className="text-lg font-semibold text-[var(--ink-primary)]">{shelf.title}</h2>
            {shelf.tracks.length > 0 ? (
              <div>
                {shelf.tracks.slice(0, 12).map((tr, idx) => (
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
        ))
      )}
    </div>
  );
}

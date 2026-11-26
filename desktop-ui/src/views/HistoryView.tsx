import { RefreshCw } from "lucide-react";
import { TrackRow } from "@/components/TrackRow";
import { Button } from "@/components/ui/button";
import { getStrings } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { useSonataStore } from "@/store/useSonataStore";

export function HistoryView() {
  const snapshot = useSonataStore((s) => s.snapshot);
  const dispatch = useSonataStore((s) => s.dispatch);
  const t = getStrings(snapshot.config.language);

  const combined =
    snapshot.history_tracks.length > 0
      ? snapshot.history_tracks
      : snapshot.last_played_tracks;

  return (
    <div className="sonata-scroll h-full overflow-y-auto px-5 @md:px-8 pt-5 pb-36 flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold tracking-tight">{t.history}</h1>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => void dispatch({ type: "RefreshHistory" })}
        >
          <RefreshCw className={cn("h-3.5 w-3.5", snapshot.history_loading && "animate-spin")} />
          {t.refresh}
        </Button>
      </div>

      {combined.length === 0 ? (
        <div className="py-12 text-sm text-[var(--ink-muted)]">
          {t.noListeningHistory}
        </div>
      ) : (
        <div>
          {combined.map((track, idx) => (
            <TrackRow
              key={`${track.id}-${idx}`}
              track={track}
              index={idx}
              allTracks={combined}
              showCover
              showAlbum
            />
          ))}
        </div>
      )}
    </div>
  );
}

import { Play } from "lucide-react";
import { ArtworkImage } from "@/components/ArtworkImage";
import { cn } from "@/lib/utils";
import { useSonataStore } from "@/store/useSonataStore";
import type { HomeCard, Track } from "@/types/sonata";

interface MediaCardProps {
  card: HomeCard;
  layout?: "carousel" | "grid";
  shelfCards?: HomeCard[];
  onClick?: () => void;
}

function cardToTrack(card: HomeCard): Track {
  if (card.track) {
    return card.track;
  }
  return {
    id: card.id,
    title: card.title,
    artist: card.subtitle,
    album: "",
    album_id: null,
    duration_secs: null,
    thumbnail_url: card.thumbnail_url ?? null,
    liked: false,
    is_explicit: false,
    source: card.source,
  };
}

function resolveCardKind(card: HomeCard) {
  if (card.kind) return card.kind;
  if (card.is_artist) return "Artist";
  if (card.is_album) return "Album";
  if (card.is_playlist) return "Playlist";
  if (card.track) return "Song";
  return "Playlist";
}

export function MediaCard({
  card,
  layout = "carousel",
  shelfCards,
  onClick,
}: MediaCardProps) {
  const dispatch = useSonataStore((s) => s.dispatch);
  const navigate = useSonataStore((s) => s.navigate);
  const homeChips = useSonataStore((s) => s.snapshot.home_chips);

  const kind = resolveCardKind(card);

  const handleOpen = () => {
    if (onClick) {
      onClick();
      return;
    }

    if (kind === "Song" || kind === "Video") {
      const clickedTrack = cardToTrack(card);
      const siblingTracks = shelfCards
        ? shelfCards
            .filter((c) => {
              const k = resolveCardKind(c);
              return k === "Song" || k === "Video";
            })
            .map(cardToTrack)
        : [];
      const startIndex = siblingTracks.findIndex((t) => t.id === clickedTrack.id);
      const tracks =
        siblingTracks.length > 0 && startIndex >= 0 ? siblingTracks : [clickedTrack];
      void dispatch({
        type: "PlayTracks",
        tracks,
        start_index: Math.max(0, startIndex),
        shuffle: false,
      });
      return;
    }

    if (kind === "Mood") {
      const chip = homeChips.find(
        (c) => c.title.trim().toLowerCase() === card.title.trim().toLowerCase(),
      );
      if (chip) {
        void dispatch({ type: "SelectHomeChip", chip });
      } else {
        void navigate({ kind: "Explore" });
      }
      return;
    }

    if (kind === "Artist") {
      void dispatch({ type: "OpenArtist", id: card.id });
    } else if (kind === "Album") {
      void dispatch({
        type: "OpenAlbum",
        album_id: card.id,
        title: card.title,
        artist: card.subtitle,
        thumbnail_url: card.thumbnail_url,
      });
    } else {
      void dispatch({
        type: "OpenCollection",
        id: card.id,
        title: card.title,
        subtitle: card.subtitle,
        thumbnail_url: card.thumbnail_url,
        editable: false,
      });
    }
  };

  return (
    <button
      type="button"
      onClick={handleOpen}
      className={cn(
        "matte-card matte-card-interactive group flex flex-col rounded-xl border border-transparent bg-[var(--bg-elevated)]/40 p-2.5 text-left hover:bg-[var(--bg-elevated)]",
        layout === "grid" ? "w-full min-w-0" : "w-[156px] @md:w-[176px] shrink-0",
      )}
    >
      <div className="relative mb-2.5 aspect-square w-full">
        <ArtworkImage
          src={card.thumbnail_url}
          alt={card.title}
          className="size-full"
          imgClassName="art-zoom"
          rounded={kind === "Artist" ? "rounded-full" : "rounded-lg"}
        />
        {kind !== "Artist" && kind !== "Mood" && (
          <span className="absolute bottom-2 right-2 flex size-8 items-center justify-center rounded-full bg-[var(--accent)] text-[var(--accent-ink)] opacity-0 shadow-lg transition-all duration-150 translate-y-1 group-hover:opacity-100 group-hover:translate-y-0">
            <Play className="size-3.5 fill-current translate-x-[0.5px]" />
          </span>
        )}
      </div>
      <div
        className={cn(
          "truncate text-[13px] font-semibold text-[var(--ink-primary)]",
          kind === "Artist" && "text-center",
        )}
      >
        {card.title}
      </div>
      {card.subtitle && (
        <div
          className={cn(
            "mt-0.5 truncate text-xs text-[var(--ink-secondary)]",
            kind === "Artist" && "text-center",
          )}
        >
          {card.subtitle}
        </div>
      )}
    </button>
  );
}

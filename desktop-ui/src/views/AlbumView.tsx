import {
  ArrowDownUp,
  BadgeCheck,
  Check,
  Download,
  MoreHorizontal,
  Music2,
  Pin,
  Play,
  Plus,
  Search,
  Shuffle,
} from "lucide-react";
import { useMemo, useState } from "react";
import { ArtworkImage } from "@/components/ArtworkImage";
import { TrackRow } from "@/components/TrackRow";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { getStrings } from "@/lib/i18n";
import { sanitizeAccentRgb, sanitizeAmbientBgRgb } from "@/lib/utils";
import { useSonataStore } from "@/store/useSonataStore";

interface AlbumViewProps {
  mode: "album" | "playlist";
}

export function AlbumView({ mode }: AlbumViewProps) {
  const snapshot = useSonataStore((s) => s.snapshot);
  const settings = useSonataStore((s) => s.settings);
  const dispatch = useSonataStore((s) => s.dispatch);
  const togglePin = useSonataStore((s) => s.togglePinCollection);

  const [filterText, setFilterText] = useState("");
  const [sortBy, setSortBy] = useState<"default" | "title" | "duration">("default");

  const t = getStrings(snapshot.config.language);

  const album = snapshot.album_detail;
  const isPlaylist = mode === "playlist";
  const loading = isPlaylist ? snapshot.playlist_loading : snapshot.album_loading;

  const id = isPlaylist ? (snapshot.playlist_id ?? "") : (album?.id ?? "");
  const title = isPlaylist ? snapshot.playlist_heading : (album?.title ?? t.albumLabel);
  const subtitle = isPlaylist ? snapshot.playlist_subtitle : (album?.artist ?? "");
  const coverUrl = isPlaylist
    ? snapshot.playlist_cover_url || snapshot.playlist_tracks[0]?.thumbnail_url
    : album?.thumbnail_url;
  const tracks = isPlaylist ? snapshot.playlist_tracks : (album?.tracks ?? []);
  const year = isPlaylist ? null : album?.year;
  const isSaved = album?.is_saved ?? false;
  const isPinned = settings.pinned_collections.some((p) => p.id === id);

  const filteredTracks = useMemo(() => {
    const q = filterText.trim().toLowerCase();
    let list = q
      ? tracks.filter(
          (tr) =>
            tr.title.toLowerCase().includes(q) ||
            tr.artist.toLowerCase().includes(q),
        )
      : [...tracks];
    if (sortBy === "title") {
      list.sort((a, b) => a.title.localeCompare(b.title));
    } else if (sortBy === "duration") {
      list.sort((a, b) => (a.duration_secs ?? 0) - (b.duration_secs ?? 0));
    }
    return list;
  }, [tracks, filterText, sortBy]);

  const [ar, ag, ab] = sanitizeAmbientBgRgb(
    settings.adaptive_art_color ? snapshot.ambient_rgb : null,
  );
  const [br, bg, bb] = sanitizeAccentRgb(
    settings.adaptive_art_color ? snapshot.ambient_rgb : null,
  );

  if (loading && tracks.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[var(--ink-secondary)]">
        {t.loading}
      </div>
    );
  }

  return (
    <div className="sonata-scroll h-full overflow-y-auto pb-36">
      {/* Matte radial ambient header matching research/theme/Screenshot-2026-11-26-181639.png */}
      <div
        className="relative px-5 @md:px-8 pt-16 pb-7"
        style={{
          background: `radial-gradient(ellipse 95% 115% at 55% 0%, rgba(${ar}, ${ag}, ${ab}, 0.72) 0%, rgba(${ar}, ${ag}, ${ab}, 0.16) 58%, transparent 100%)`,
        }}
      >
        <div className="hero-enter flex flex-col @md:flex-row items-start @md:items-end gap-5 @md:gap-7">
          <ArtworkImage
            src={coverUrl}
            alt={title}
            className="size-36 @md:size-48 @lg:size-52 shrink-0 shadow-2xl border-white/15"
            rounded="rounded-2xl"
          />

          <div className="min-w-0 flex-1 flex flex-col gap-3">
            <h1 className="text-2xl @md:text-3xl @lg:text-4xl font-bold tracking-tight text-[var(--ink-primary)] line-clamp-2">
              {title || t.collectionLabel}
            </h1>

            {subtitle && (
              <button
                type="button"
                onClick={() => {
                  if (!isPlaylist && album?.artist_id) {
                    void dispatch({ type: "OpenArtist", id: album.artist_id });
                  } else if (!isPlaylist && album?.artist) {
                    void dispatch({ type: "OpenArtistByName", name: album.artist });
                  }
                }}
                className="inline-flex items-center gap-2 text-sm font-medium text-[var(--ink-primary)]/90 hover:underline"
              >
                <span>{subtitle}</span>
                {!isPlaylist && <BadgeCheck className="h-4 w-4 text-[var(--ink-secondary)]" />}
              </button>
            )}

            <div className="flex flex-wrap items-center gap-2">
              {year && <Badge>{year}</Badge>}
              <Badge>
                <Music2 className="h-3 w-3" />
                {tracks.length} {t.tracks}
              </Badge>
            </div>

            <div className="flex flex-wrap items-center justify-between gap-3 pt-2">
              <div className="flex items-center gap-2.5">
                <Button
                  size="pill"
                  disabled={tracks.length === 0}
                  onClick={() =>
                    void dispatch({
                      type: "PlayTracks",
                      tracks,
                      start_index: 0,
                      shuffle: false,
                    })
                  }
                  style={{
                    backgroundColor: `rgb(${br}, ${bg}, ${bb})`,
                    color: "#111113",
                  }}
                  className="font-semibold"
                >
                  <Play className="h-4 w-4 fill-current" />
                  {t.play}
                </Button>

                <Button
                  size="pill"
                  disabled={tracks.length === 0}
                  onClick={() =>
                    void dispatch({
                      type: "PlayTracks",
                      tracks,
                      start_index: 0,
                      shuffle: true,
                    })
                  }
                  style={{
                    backgroundColor: `rgb(${br}, ${bg}, ${bb})`,
                    color: "#111113",
                  }}
                  className="font-semibold"
                >
                  <Shuffle className="h-4 w-4" />
                  {t.shuffle}
                </Button>
              </div>

              <div className="flex items-center gap-2">
                {!isPlaylist && album && (
                  <Button
                    size="pill"
                    variant="secondary"
                    onClick={() => void dispatch({ type: "ToggleSaveAlbum" })}
                  >
                    {isSaved ? <Check className="h-4 w-4" /> : <Plus className="h-4 w-4" />}
                    {isSaved ? t.saved : t.add}
                  </Button>
                )}

                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button size="icon" variant="secondary" className="rounded-full" aria-label={t.moreOptions}>
                      <MoreHorizontal className="h-4 w-4" />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    {id && (
                      <DropdownMenuItem
                        onSelect={() =>
                          void togglePin({
                            id,
                            title,
                            subtitle,
                            thumbnail_url: coverUrl,
                            is_album: !isPlaylist,
                          })
                        }
                      >
                        <Pin className="h-3.5 w-3.5" />
                        {isPinned ? t.unpinFromSidebar : t.pinToSidebar}
                      </DropdownMenuItem>
                    )}
                    {tracks.length > 0 && (
                      <DropdownMenuItem
                        onSelect={() => {
                          tracks.forEach((tr) => {
                            void dispatch({
                              type: "DownloadTrack",
                              track: tr,
                              collection_id: id || null,
                              collection_title: title || null,
                            });
                          });
                        }}
                      >
                        <Download className="h-3.5 w-3.5" />
                        {t.downloadAll}
                      </DropdownMenuItem>
                    )}
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Inline filter + sort bar */}
      <div className="flex flex-wrap items-center justify-between gap-3 px-5 @md:px-8 pb-3">
        <div className="relative w-full max-w-64">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[var(--ink-muted)]" />
          <input
            type="text"
            value={filterText}
            onChange={(e) => setFilterText(e.target.value)}
            placeholder={t.filterTracks}
            className="h-8 w-full rounded-lg border border-transparent bg-transparent pl-8 pr-3 text-xs text-[var(--ink-primary)] placeholder:text-[var(--ink-muted)] hover:bg-[var(--bg-elevated)] focus:bg-[var(--bg-elevated)] focus:border-[var(--border-hairline)] focus:outline-none"
          />
        </div>

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button size="sm" variant="ghost" aria-label={t.sortTracks}>
              <ArrowDownUp className="h-3.5 w-3.5" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onSelect={() => setSortBy("default")}>
              {t.sortOrder}
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => setSortBy("title")}>
              {t.sortTitle}
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => setSortBy("duration")}>
              {t.sortDuration}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      {/* Track table */}
      <div className="px-3 @md:px-6">
        {filteredTracks.length === 0 ? (
          <div className="py-12 text-center text-sm text-[var(--ink-muted)]">
            {t.noMatchingTracks}
          </div>
        ) : (
          filteredTracks.map((track, idx) => (
            <TrackRow
              key={`${track.id}-${idx}`}
              track={track}
              index={idx}
              allTracks={filteredTracks}
              showCover={isPlaylist}
              showAlbum={isPlaylist}
              removableFromPlaylist={isPlaylist && snapshot.playlist_editable}
            />
          ))
        )}
      </div>
    </div>
  );
}

import {
  Check,
  ChevronLeft,
  ChevronRight,
  Download,
  MoreHorizontal,
  Play,
  Radio,
  Star,
} from "lucide-react";
import { useEffect, useState } from "react";
import { ArtworkImage } from "@/components/ArtworkImage";
import { HorizontalScroller } from "@/components/HorizontalScroller";
import { MediaCard } from "@/components/MediaCard";
import { TrackRow } from "@/components/TrackRow";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { getStrings } from "@/lib/i18n";
import { resolveArtworkSrc } from "@/lib/ipc";
import { cn, sanitizeAccentRgb } from "@/lib/utils";
import { useSonataStore } from "@/store/useSonataStore";

export function ArtistView() {
  const snapshot = useSonataStore((s) => s.snapshot);
  const settings = useSonataStore((s) => s.settings);
  const dispatch = useSonataStore((s) => s.dispatch);
  const togglePin = useSonataStore((s) => s.togglePinCollection);

  const [topSongPage, setTopSongPage] = useState(0);
  const t = getStrings(snapshot.config.language);

  const artist = snapshot.artist_page;
  useEffect(() => {
    setTopSongPage(0);
  }, [artist?.id]);
  if (snapshot.artist_loading && !artist) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[var(--ink-secondary)]">
        {t.loadingArtist}
      </div>
    );
  }
  if (!artist) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-[var(--ink-muted)]">
        {snapshot.artist_error || t.selectArtistPrompt}
      </div>
    );
  }

  const heroSrc = resolveArtworkSrc(
    snapshot.artist_banner_url ||
      artist.banner_url ||
      artist.thumbnail_url ||
      artist.top_tracks[0]?.thumbnail_url ||
      artist.albums[0]?.thumbnail_url ||
      artist.singles[0]?.thumbnail_url,
  );
  const latestRelease = artist.albums[0] || artist.singles[0];
  const topSongs = artist.top_tracks;
  const isLibraryTracksOnly =
    !latestRelease && artist.albums.length === 0 && artist.singles.length === 0;
  const pageCount = Math.max(1, Math.ceil(topSongs.length / 6));
  const pagedSongs = topSongs.slice(topSongPage * 6, topSongPage * 6 + 6);
  const isPinned = settings.pinned_collections.some((p) => p.id === artist.id);
  const [br, bg, bb] = sanitizeAccentRgb(snapshot.ambient_rgb);

  const downloadAllArtistSongs = () => {
    topSongs.forEach((track) => {
      void dispatch({
        type: "DownloadTrack",
        track,
        collection_id: artist.id,
        collection_title: artist.name,
      });
    });
  };

  return (
    <div className="sonata-scroll h-full overflow-y-auto pb-36">
      {/* Full-bleed edge-to-edge hero matching research/theme/Screenshot-2026-11-26-181626.png */}
      <div className="relative h-[280px] @lg:h-[340px] w-full overflow-hidden">
        {heroSrc ? (
          <img
            src={heroSrc}
            alt={artist.name}
            className="h-full w-full object-cover object-center"
          />
        ) : (
          <div className="h-full w-full bg-[var(--bg-elevated)]" />
        )}
        <div
          className="absolute inset-0"
          style={{
            background:
              "linear-gradient(180deg, rgba(17,17,20,0.18) 0%, rgba(17,17,20,0.52) 60%, var(--bg-surface) 100%)",
          }}
        />

        <div className="hero-enter absolute inset-x-0 bottom-5 px-5 @md:px-8">
          <div className="mb-2.5 inline-flex items-center gap-1.5 rounded-full bg-white/15 px-3 py-1 text-xs font-medium text-[var(--ink-primary)] border border-white/15">
            <span>{artist.subscribers || t.artistLabel}</span>
          </div>
          <h1 className="text-3xl @md:text-4xl @lg:text-5xl font-bold tracking-tight text-[var(--ink-primary)]">
            {artist.name}
          </h1>

          <div className="mt-4 flex items-center gap-2.5">
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <button
                  type="button"
                  aria-label={t.artistOptions}
                  className="matte-btn flex h-10 w-10 items-center justify-center rounded-full border border-white/15 bg-black/45 text-[var(--ink-primary)] hover:bg-black/65"
                >
                  <MoreHorizontal className="h-4 w-4" />
                </button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="start">
                {topSongs[0] && (
                  <DropdownMenuItem
                    onSelect={() => void dispatch({ type: "StartTrackRadio", track: topSongs[0] })}
                  >
                    <Radio className="h-3.5 w-3.5" />
                    {t.startRadio}
                  </DropdownMenuItem>
                )}
                <DropdownMenuItem
                  onSelect={() =>
                    void togglePin({
                      id: artist.id,
                      title: artist.name,
                      subtitle: t.artistLabel,
                      thumbnail_url: artist.thumbnail_url,
                      is_album: false,
                    })
                  }
                >
                  <Star className="h-3.5 w-3.5" />
                  {isPinned ? t.unpinFromSidebar : t.pinToSidebar}
                </DropdownMenuItem>
                {topSongs.length > 0 && (
                  <DropdownMenuItem onSelect={downloadAllArtistSongs}>
                    <Download className="h-3.5 w-3.5" />
                    {t.downloadAll}
                  </DropdownMenuItem>
                )}
              </DropdownMenuContent>
            </DropdownMenu>

            <button
              type="button"
              disabled={topSongs.length === 0}
              onClick={() =>
                void dispatch({
                  type: "PlayTracks",
                  tracks: topSongs,
                  start_index: 0,
                  shuffle: false,
                })
              }
              aria-label={`${t.play} ${artist.name}`}
              style={{
                backgroundColor: `rgb(${br}, ${bg}, ${bb})`,
                color: "#111113",
              }}
              className="matte-btn flex h-11 w-11 items-center justify-center rounded-full font-semibold shadow-lg disabled:opacity-40"
            >
              <Play className="h-5 w-5 fill-current translate-x-[0.5px]" />
            </button>

            <button
              type="button"
              onClick={() =>
                void togglePin({
                  id: artist.id,
                  title: artist.name,
                  subtitle: t.artistLabel,
                  thumbnail_url: artist.thumbnail_url,
                  is_album: false,
                })
              }
              aria-label={t.pinArtist}
              className="matte-btn flex h-10 w-10 items-center justify-center rounded-full border border-white/15 bg-black/45 text-[var(--ink-primary)] hover:bg-black/65"
            >
              <Star className={isPinned ? "h-4 w-4 fill-current text-[var(--accent)]" : "h-4 w-4"} />
            </button>

            {topSongs.length > 0 && (
              <button
                type="button"
                onClick={downloadAllArtistSongs}
                aria-label={t.downloadAll}
                title={t.downloadAll}
                className="matte-btn flex h-10 w-10 items-center justify-center rounded-full border border-white/15 bg-black/45 text-[var(--ink-primary)] hover:bg-black/65"
              >
                <Download className="h-4 w-4" />
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Latest Release (left when present) + Top Songs */}
      <div
        className={cn(
          "grid grid-cols-1 gap-6 px-5 @md:px-8 pt-4",
          latestRelease && "@xl:grid-cols-[248px_1fr]",
        )}
      >
        {latestRelease && (
          <section>
            <h2 className="mb-3 text-sm font-semibold text-[var(--ink-primary)]">
              {t.latestRelease}
            </h2>
            <button
              type="button"
              onClick={() =>
                void dispatch({
                  type: "OpenAlbum",
                  album_id: latestRelease.id,
                  title: latestRelease.title,
                  artist: artist.name,
                  thumbnail_url: latestRelease.thumbnail_url,
                })
              }
              className="matte-card matte-card-interactive flex w-full items-center gap-3.5 rounded-2xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] p-3 text-left"
            >
              <ArtworkImage
                src={latestRelease.thumbnail_url}
                alt={latestRelease.title}
                className="h-20 w-20"
                imgClassName="art-zoom"
                rounded="rounded-xl"
              />
              <div className="min-w-0 flex-1">
                <div className="text-[11px] font-semibold uppercase tracking-wider text-[var(--ink-muted)]">
                  {latestRelease.subtitle || t.releaseLabel}
                </div>
                <div className="mt-1 truncate text-sm font-semibold text-[var(--ink-primary)]">
                  {latestRelease.title}
                </div>
              </div>
            </button>
          </section>
        )}

        <section className="min-w-0">
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-sm font-semibold text-[var(--ink-primary)] flex items-center gap-1">
              {t.topSongs}
              <ChevronRight className="h-4 w-4 text-[var(--ink-muted)]" />
            </h2>
            {!isLibraryTracksOnly && pageCount > 1 && (
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  disabled={topSongPage === 0}
                  onClick={() => setTopSongPage((p) => Math.max(0, p - 1))}
                  aria-label={t.previousTopSongs}
                  className="flex h-7 w-7 items-center justify-center rounded-full border border-[var(--border-hairline)] bg-[var(--bg-elevated)] text-[var(--ink-secondary)] hover:text-[var(--ink-primary)] disabled:opacity-35"
                >
                  <ChevronLeft className="h-4 w-4" />
                </button>
                <button
                  type="button"
                  disabled={topSongPage >= pageCount - 1}
                  onClick={() => setTopSongPage((p) => Math.min(pageCount - 1, p + 1))}
                  aria-label={t.nextTopSongs}
                  className="flex h-7 w-7 items-center justify-center rounded-full border border-[var(--border-hairline)] bg-[var(--bg-elevated)] text-[var(--ink-secondary)] hover:text-[var(--ink-primary)] disabled:opacity-35"
                >
                  <ChevronRight className="h-4 w-4" />
                </button>
              </div>
            )}
          </div>

          {isLibraryTracksOnly ? (
            <div className="rounded-xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)]/30">
              {topSongs.map((track, i) => (
                <TrackRow
                  key={`${track.id}-${i}`}
                  track={track}
                  index={i}
                  allTracks={topSongs}
                  showCover
                  showAlbum
                />
              ))}
            </div>
          ) : (
            <div className="grid grid-cols-1 @md:grid-cols-2 @2xl:grid-cols-3 gap-x-4 gap-y-1.5">
              {pagedSongs.map((track, i) => {
                const globalIdx = topSongPage * 6 + i;
                const isDownloaded = snapshot.downloaded_track_ids.includes(track.id);
                return (
                  <div
                    key={`${track.id}-${globalIdx}`}
                    className="group flex items-center gap-2 rounded-lg p-1.5 hover:bg-[var(--bg-elevated)] transition-colors"
                  >
                    <button
                      type="button"
                      onClick={() =>
                        void dispatch({
                          type: "PlayTracks",
                          tracks: topSongs,
                          start_index: globalIdx,
                          shuffle: false,
                        })
                      }
                      className="flex min-w-0 flex-1 items-center gap-2.5 text-left"
                    >
                      <ArtworkImage
                        src={track.thumbnail_url}
                        alt={track.title}
                        className="h-10 w-10"
                        rounded="rounded-md"
                      />
                      <div className="min-w-0 flex-1">
                        <div className="truncate text-[13px] font-medium text-[var(--ink-primary)]">
                          {track.title}
                        </div>
                        <div className="truncate text-[11px] text-[var(--ink-muted)]">
                          {track.album || track.artist}
                        </div>
                      </div>
                    </button>
                    <button
                      type="button"
                      aria-label={isDownloaded ? t.downloaded : t.download}
                      title={isDownloaded ? t.downloaded : t.download}
                      onClick={() =>
                        void dispatch({
                          type: "DownloadTrack",
                          track,
                          collection_id: artist.id,
                          collection_title: artist.name,
                        })
                      }
                      className={cn(
                        "flex h-7 w-7 shrink-0 items-center justify-center rounded-md transition-opacity",
                        isDownloaded
                          ? "text-[var(--accent)] opacity-100"
                          : "text-[var(--ink-muted)] opacity-0 group-hover:opacity-100 hover:text-[var(--ink-primary)]",
                      )}
                    >
                      {isDownloaded ? (
                        <Check className="h-3.5 w-3.5" />
                      ) : (
                        <Download className="h-3.5 w-3.5" />
                      )}
                    </button>
                  </div>
                );
              })}
            </div>
          )}
        </section>
      </div>

      {/* Discography & Related Artists */}
      {artist.albums.length > 0 && (
        <section className="px-5 @md:px-8 pt-7">
          <h2 className="mb-3 text-base font-semibold text-[var(--ink-primary)]">{t.albums}</h2>
          <HorizontalScroller contentClassName="gap-3 py-1">
            {artist.albums.map((card) => (
              <MediaCard key={card.id} card={card} />
            ))}
          </HorizontalScroller>
        </section>
      )}

      {artist.singles.length > 0 && (
        <section className="px-5 @md:px-8 pt-6">
          <h2 className="mb-3 text-base font-semibold text-[var(--ink-primary)]">{t.singlesAndEps}</h2>
          <HorizontalScroller contentClassName="gap-3 py-1">
            {artist.singles.map((card) => (
              <MediaCard key={card.id} card={card} />
            ))}
          </HorizontalScroller>
        </section>
      )}

      {artist.related_artists.length > 0 && (
        <section className="px-5 @md:px-8 pt-6">
          <h2 className="mb-3 text-base font-semibold text-[var(--ink-primary)]">{t.relatedArtists}</h2>
          <HorizontalScroller contentClassName="gap-3 py-1">
            {artist.related_artists.map((card) => (
              <MediaCard key={card.id} card={card} />
            ))}
          </HorizontalScroller>
        </section>
      )}
    </div>
  );
}

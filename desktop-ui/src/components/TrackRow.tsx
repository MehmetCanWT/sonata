import {
  Check,
  Disc3,
  Download,
  Heart,
  ListPlus,
  MoreHorizontal,
  Play,
  PlusCircle,
  Radio,
  Trash2,
  User,
} from "lucide-react";
import { ArtworkImage } from "@/components/ArtworkImage";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { getStrings } from "@/lib/i18n";
import { cn, formatDuration } from "@/lib/utils";
import { useSonataStore } from "@/store/useSonataStore";
import type { Track } from "@/types/sonata";

interface TrackRowProps {
  track: Track;
  index: number;
  allTracks: Track[];
  showCover?: boolean;
  showAlbum?: boolean;
  removableFromPlaylist?: boolean;
}

export function TrackRow({
  track,
  index,
  allTracks,
  showCover = false,
  showAlbum = false,
  removableFromPlaylist = false,
}: TrackRowProps) {
  const snapshot = useSonataStore((s) => s.snapshot);
  const dispatch = useSonataStore((s) => s.dispatch);
  const setModal = useSonataStore((s) => s.setModal);

  const t = getStrings(snapshot.config.language);
  const currentTrack =
    snapshot.current_index != null ? snapshot.queue[snapshot.current_index] : undefined;
  const isPlaying = currentTrack?.id === track.id;
  const isLiked = track.liked || snapshot.liked_track_ids.includes(track.id);
  const isDownloaded = snapshot.downloaded_track_ids.includes(track.id);

  const handlePlay = () => {
    void dispatch({
      type: "PlayTracks",
      tracks: allTracks,
      start_index: index,
      shuffle: false,
    });
  };

  const renderMenuItems = (MenuItemComp: typeof ContextMenuItem, SepComp: typeof ContextMenuSeparator) => (
    <>
      <MenuItemComp onSelect={handlePlay}>
        <Play className="h-3.5 w-3.5" />
        {t.play}
      </MenuItemComp>
      <MenuItemComp onSelect={() => void dispatch({ type: "PlayTrackNext", track })}>
        <PlusCircle className="h-3.5 w-3.5" />
        {t.playNext}
      </MenuItemComp>
      <MenuItemComp onSelect={() => void dispatch({ type: "AddTrackToQueue", track })}>
        <ListPlus className="h-3.5 w-3.5" />
        {t.addToQueue}
      </MenuItemComp>
      <MenuItemComp onSelect={() => setModal({ type: "addToPlaylist", track })}>
        <ListPlus className="h-3.5 w-3.5" />
        {t.addToPlaylist}
      </MenuItemComp>
      <MenuItemComp onSelect={() => void dispatch({ type: "StartTrackRadio", track })}>
        <Radio className="h-3.5 w-3.5" />
        {t.startRadio}
      </MenuItemComp>
      <SepComp />
      <MenuItemComp onSelect={() => void dispatch({ type: "ToggleLike", track })}>
        <Heart className={cn("h-3.5 w-3.5", isLiked && "fill-current text-[var(--accent)]")} />
        {isLiked ? t.unlike : t.like}
      </MenuItemComp>
      {track.album && (
        <MenuItemComp
          onSelect={() =>
            void dispatch({
              type: "OpenAlbum",
              album_id: track.album_id,
              title: track.album,
              artist: track.artist,
              thumbnail_url: track.thumbnail_url,
            })
          }
        >
          <Disc3 className="h-3.5 w-3.5" />
          {t.goToAlbum}
        </MenuItemComp>
      )}
      {track.artist && (
        <MenuItemComp onSelect={() => void dispatch({ type: "OpenArtistByName", name: track.artist })}>
          <User className="h-3.5 w-3.5" />
          {t.goToArtist}
        </MenuItemComp>
      )}
      <SepComp />
      {isDownloaded ? (
        <MenuItemComp onSelect={() => void dispatch({ type: "RemoveDownloadedTrack", track_id: track.id })}>
          <Trash2 className="h-3.5 w-3.5 text-red-400" />
          {t.removeDownload}
        </MenuItemComp>
      ) : (
        <MenuItemComp onSelect={() => void dispatch({ type: "DownloadTrack", track })}>
          <Download className="h-3.5 w-3.5" />
          {t.download}
        </MenuItemComp>
      )}
      {removableFromPlaylist && (
        <MenuItemComp onSelect={() => void dispatch({ type: "RemoveTrackFromPlaylist", index })}>
          <Trash2 className="h-3.5 w-3.5 text-red-400" />
          {t.removeFromPlaylist}
        </MenuItemComp>
      )}
    </>
  );

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <div
          onDoubleClick={handlePlay}
          className={cn(
            "group flex items-center gap-3 border-b border-[var(--border-hairline)] px-3 py-2.5 text-sm transition-colors hover:bg-[var(--bg-elevated)]/70",
            isPlaying && "bg-[var(--bg-elevated)]",
          )}
        >
          {/* Index / Hover Play */}
          <button
            type="button"
            onClick={handlePlay}
            aria-label={`${t.play} ${track.title}`}
            className="flex h-6 w-7 shrink-0 items-center justify-center text-xs font-medium text-[var(--ink-muted)] tabular-nums"
          >
            <span className={cn("group-hover:hidden", isPlaying && "text-[var(--accent)] font-semibold")}>
              {index + 1}
            </span>
            <Play className="hidden h-3.5 w-3.5 fill-current text-[var(--ink-primary)] group-hover:block" />
          </button>

          {showCover && (
            <ArtworkImage
              src={track.thumbnail_url}
              alt={track.title}
              className="h-9 w-9"
              rounded="rounded-md"
            />
          )}

          {/* Title + Artist */}
          <button
            type="button"
            onClick={handlePlay}
            className="min-w-0 flex-1 text-left"
          >
            <div className="flex items-center gap-2 min-w-0">
              <span
                className={cn(
                  "truncate font-medium",
                  isPlaying ? "text-[var(--accent)]" : "text-[var(--ink-primary)]",
                )}
              >
                {track.title}
              </span>
              {isLiked && <Check className="h-3.5 w-3.5 shrink-0 text-[var(--accent)]" />}
            </div>
            {(showCover || track.artist) && (
              <div className="truncate text-xs text-[var(--ink-secondary)]">
                {track.artist}
              </div>
            )}
          </button>

          {showAlbum && track.album && (
            <button
              type="button"
              onClick={() =>
                void dispatch({
                  type: "OpenAlbum",
                  album_id: track.album_id,
                  title: track.album,
                  artist: track.artist,
                  thumbnail_url: track.thumbnail_url,
                })
              }
              className="hidden @xl:block w-44 truncate text-left text-xs text-[var(--ink-secondary)] hover:text-[var(--ink-primary)]"
            >
              {track.album}
            </button>
          )}

          {/* Right metadata + duration + actions */}
          <div className="flex items-center gap-3 shrink-0">
            <span className="w-11 text-right text-xs text-[var(--ink-muted)] tabular-nums">
              {formatDuration(track.duration_secs)}
            </span>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <button
                  type="button"
                  aria-label={t.trackOptions}
                  className="flex size-7 items-center justify-center rounded-md text-[var(--ink-muted)] opacity-60 @md:opacity-0 group-hover:opacity-100 focus-visible:opacity-100 hover:bg-[var(--bg-hover)] hover:text-[var(--ink-primary)] transition-opacity"
                >
                  <MoreHorizontal className="h-4 w-4" />
                </button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                {renderMenuItems(DropdownMenuItem, DropdownMenuSeparator)}
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent>
        {renderMenuItems(ContextMenuItem, ContextMenuSeparator)}
      </ContextMenuContent>
    </ContextMenu>
  );
}

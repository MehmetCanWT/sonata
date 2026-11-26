import {
  Activity,
  FolderPlus,
  HardDrive,
  PinOff,
  Plus,
  RefreshCw,
  Server,
  Trash2,
} from "lucide-react";
import { useState } from "react";
import { MediaCard } from "@/components/MediaCard";
import { TrackRow } from "@/components/TrackRow";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { getStrings } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { type LibrarySection, useSonataStore } from "@/store/useSonataStore";

export function LibraryView() {
  const snapshot = useSonataStore((s) => s.snapshot);
  const settings = useSonataStore((s) => s.settings);
  const librarySection = useSonataStore((s) => s.librarySection);
  const setLibrarySection = useSonataStore((s) => s.setLibrarySection);
  const togglePin = useSonataStore((s) => s.togglePinCollection);
  const setModal = useSonataStore((s) => s.setModal);
  const dispatch = useSonataStore((s) => s.dispatch);

  const [newDirInput, setNewDirInput] = useState("");
  const t = getStrings(snapshot.config.language);

  const sections: Array<{ id: LibrarySection; label: string }> = [
    { id: "albums", label: t.albums },
    { id: "artists", label: t.artists },
    { id: "all_playlists", label: t.playlists },
    { id: "songs", label: t.songs },
    { id: "pins", label: t.pins },
    { id: "recent", label: t.recentlyAdded },
    { id: "downloads", label: t.downloads },
    { id: "local_servers", label: t.localAndServers },
  ];

  const downloadedAsTracks = snapshot.downloaded_tracks.map((dt) => ({
    id: dt.track_id,
    title: dt.title,
    artist: dt.artist,
    album: dt.album,
    duration_secs: dt.duration_secs,
    thumbnail_url: dt.artwork_path || null,
    liked: snapshot.liked_track_ids.includes(dt.track_id),
    audio_badge: dt.badge,
    is_explicit: false,
    source: "YouTubeMusic" as const,
  }));

  return (
    <div className="sonata-scroll h-full overflow-y-auto px-5 @md:px-8 pt-5 pb-36 flex flex-col gap-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-3xl font-bold tracking-tight">{t.library}</h1>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => void dispatch({ type: "RefreshLibrary" })}
        >
          <RefreshCw
            className={cn("h-3.5 w-3.5", snapshot.library_extras_loading && "animate-spin")}
          />
          {t.refresh}
        </Button>
      </div>

      <Tabs
        value={librarySection}
        onValueChange={(v) => setLibrarySection(v as LibrarySection)}
      >
        <TabsList className="flex-wrap h-auto">
          {sections.map((sec) => (
            <TabsTrigger key={sec.id} value={sec.id}>
              {sec.label}
            </TabsTrigger>
          ))}
        </TabsList>
      </Tabs>

      {librarySection === "pins" && (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(156px,1fr))] gap-3">
          {settings.pinned_collections.length === 0 ? (
            <p className="col-span-full text-sm text-[var(--ink-muted)]">
              {t.noPinsYet}
            </p>
          ) : (
            settings.pinned_collections.map((pin) => (
              <div key={pin.id} className="relative group">
                <MediaCard
                  layout="grid"
                  card={{
                    id: pin.id,
                    title: pin.title,
                    subtitle: pin.subtitle,
                    thumbnail_url: pin.thumbnail_url,
                    is_album: pin.is_album,
                    is_playlist: !pin.is_album && pin.subtitle !== "Artist",
                    is_artist: pin.subtitle === "Artist",
                    source: "YouTubeMusic",
                  }}
                />
                <button
                  type="button"
                  aria-label={t.unpin}
                  onClick={() => void togglePin(pin)}
                  className="absolute top-3 right-3 hidden group-hover:flex h-7 w-7 items-center justify-center rounded-full bg-black/70 text-white"
                >
                  <PinOff className="h-3.5 w-3.5" />
                </button>
              </div>
            ))
          )}
        </div>
      )}

      {librarySection === "albums" && (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(156px,1fr))] gap-3">
          {snapshot.library_albums.map((card) => (
            <MediaCard key={card.id} card={card} layout="grid" />
          ))}
          {snapshot.library_albums.length === 0 && (
            <p className="col-span-full text-sm text-[var(--ink-muted)]">
              {t.signInAlbumsPrompt}
            </p>
          )}
        </div>
      )}

      {librarySection === "artists" && (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(156px,1fr))] gap-3">
          {snapshot.library_artists.map((card) => (
            <MediaCard key={card.id} card={card} layout="grid" />
          ))}
          {snapshot.library_artists.length === 0 && (
            <p className="col-span-full text-sm text-[var(--ink-muted)]">
              {t.followArtistsPrompt}
            </p>
          )}
        </div>
      )}

      {librarySection === "all_playlists" && (
        <div className="space-y-4">
          <div className="flex items-center gap-2">
            <Button
              size="sm"
              variant="secondary"
              onClick={() => setModal({ type: "createPlaylist", kind: "personal" })}
            >
              <Plus className="h-3.5 w-3.5" />
              {t.newLocalPlaylist}
            </Button>
            {snapshot.signed_in_browser && (
              <Button
                size="sm"
                variant="secondary"
                onClick={() => setModal({ type: "createPlaylist", kind: "ytm" })}
              >
                <Plus className="h-3.5 w-3.5" />
                {t.newYtmPlaylist}
              </Button>
            )}
          </div>
          <div className="grid grid-cols-[repeat(auto-fill,minmax(156px,1fr))] gap-3">
            {snapshot.personal_playlists.map((pl) => (
              <MediaCard
                key={pl.id}
                layout="grid"
                card={{
                  id: pl.id,
                  title: pl.name,
                  subtitle: `${pl.tracks.length} ${t.tracks} • ${t.localTag}`,
                  thumbnail_url: pl.tracks[0]?.thumbnail_url,
                  is_album: false,
                  is_playlist: true,
                  is_artist: false,
                  source: "YouTubeMusic",
                }}
              />
            ))}
            {snapshot.ytm_playlists.map((pl) => (
              <MediaCard
                key={pl.id}
                layout="grid"
                card={{
                  id: pl.id,
                  title: pl.name,
                  subtitle: pl.track_count ? `${pl.track_count} ${t.tracks}` : t.playlistLabel,
                  thumbnail_url: pl.thumbnail_url,
                  is_album: false,
                  is_playlist: true,
                  is_artist: false,
                  source: pl.source,
                }}
              />
            ))}
          </div>
        </div>
      )}

      {(librarySection === "songs" || librarySection === "recent") && (
        <div>
          {(librarySection === "recent"
            ? snapshot.last_played_tracks
            : [...snapshot.local_tracks, ...snapshot.last_played_tracks]
          ).map((track, idx, arr) => (
            <TrackRow
              key={`${track.id}-${idx}`}
              track={track}
              index={idx}
              allTracks={arr}
              showCover
              showAlbum
            />
          ))}
        </div>
      )}

      {librarySection === "downloads" && (
        <div className="space-y-4">
          <div className="flex items-center justify-between rounded-xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] p-3.5">
            <div className="text-xs text-[var(--ink-secondary)]">
              {snapshot.downloaded_tracks.length} {t.offlineTracksSuffix} (
              {(snapshot.downloads_total_bytes / (1024 * 1024)).toFixed(1)} {t.usedSuffix})
            </div>
            {snapshot.downloaded_tracks.length > 0 && (
              <Button
                size="sm"
                variant="destructive"
                onClick={() => void dispatch({ type: "ClearAllDownloads" })}
              >
                <Trash2 className="h-3.5 w-3.5" />
                {t.clearDownloads}
              </Button>
            )}
          </div>
          {downloadedAsTracks.map((tr, idx) => (
            <TrackRow
              key={`${tr.id}-${idx}`}
              track={tr}
              index={idx}
              allTracks={downloadedAsTracks}
              showCover
              showAlbum
            />
          ))}
        </div>
      )}

      {librarySection === "local_servers" && (
        <div className="space-y-6">
          {/* Local directories */}
          <section className="rounded-2xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] p-4 space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2 text-sm font-semibold">
                <HardDrive className="h-4 w-4 text-[var(--accent)]" />
                <span>{t.localMusicDirs} ({snapshot.local_tracks.length} {t.tracks})</span>
              </div>
              <Button
                size="sm"
                variant="secondary"
                onClick={() => void dispatch({ type: "ScanLocalMusic" })}
              >
                <RefreshCw className={cn("h-3.5 w-3.5", snapshot.local_scanning && "animate-spin")} />
                {t.scan}
              </Button>
            </div>
            <div className="flex items-center gap-2">
              <Input
                value={newDirInput}
                onChange={(e) => setNewDirInput(e.target.value)}
                placeholder={t.folderPathPlaceholder}
                className="max-w-md"
              />
              <Button
                size="sm"
                variant="secondary"
                onClick={() => {
                  const path = newDirInput.trim();
                  if (!path) return;
                  void dispatch({ type: "AddLocalMusicDir", path });
                  setNewDirInput("");
                }}
              >
                <FolderPlus className="h-3.5 w-3.5" />
                {t.addFolder}
              </Button>
            </div>
            <div className="flex flex-wrap gap-2">
              {snapshot.config.local_music_dirs.map((dir) => (
                <Badge key={dir} className="normal-case gap-2 py-1">
                  <span>{dir}</span>
                  <button
                    type="button"
                    onClick={() => void dispatch({ type: "RemoveLocalMusicDir", path: dir })}
                    className="hover:text-red-400"
                  >
                    ×
                  </button>
                </Badge>
              ))}
            </div>
          </section>

          {/* Self-hosted Servers */}
          <section className="rounded-2xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] p-4 space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2 text-sm font-semibold">
                <Server className="h-4 w-4 text-[var(--accent)]" />
                <span>{t.selfHostedServers}</span>
              </div>
              <Button
                size="sm"
                variant="secondary"
                onClick={() => setModal({ type: "serverEditor" })}
              >
                <Plus className="h-3.5 w-3.5" />
                {t.addServer}
              </Button>
            </div>
            <div className="space-y-2">
              {snapshot.server_profiles.map((srv) => {
                const health = snapshot.server_health[srv.id];
                const speed = snapshot.server_speed_tests[srv.id];
                return (
                  <div
                    key={srv.id}
                    className="flex flex-wrap items-center justify-between gap-2 rounded-xl border border-[var(--border-hairline)] bg-[var(--bg-surface)] px-3.5 py-2.5"
                  >
                    <div>
                      <div className="flex items-center gap-2 text-sm font-semibold">
                        <span>{srv.name}</span>
                        <Badge>{srv.kind}</Badge>
                        {health && (
                          <Badge>{health.alive ? `${health.latency_ms ?? 0}ms` : t.offlineStatus}</Badge>
                        )}
                        {speed?.download_mbps && (
                          <Badge>{speed.download_mbps.toFixed(1)} Mbps</Badge>
                        )}
                      </div>
                      <div className="text-xs text-[var(--ink-muted)]">{srv.base_url}</div>
                    </div>
                    <div className="flex items-center gap-1.5">
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => void dispatch({ type: "SelectServer", server_id: srv.id })}
                      >
                        {t.browse}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => void dispatch({ type: "ProbeServerHealth", id: srv.id })}
                      >
                        <Activity className="h-3.5 w-3.5" />
                        {t.ping}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => void dispatch({ type: "RunServerSpeedTest", id: srv.id })}
                      >
                        {t.speed}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => void dispatch({ type: "RemoveServerProfile", id: srv.id })}
                      >
                        <Trash2 className="h-3.5 w-3.5 text-red-400" />
                      </Button>
                    </div>
                  </div>
                );
              })}
            </div>
          </section>

          {snapshot.server_tracks.length > 0 && (
            <section className="space-y-2">
              <h2 className="text-base font-semibold">{t.serverTracks}</h2>
              {snapshot.server_tracks.map((tr, idx) => (
                <TrackRow
                  key={`${tr.id}-${idx}`}
                  track={tr}
                  index={idx}
                  allTracks={snapshot.server_tracks}
                  showCover
                  showAlbum
                />
              ))}
            </section>
          )}
        </div>
      )}
    </div>
  );
}

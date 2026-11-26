import { Play, Search } from "lucide-react";
import { useState } from "react";
import { ArtworkImage } from "@/components/ArtworkImage";
import { MediaCard } from "@/components/MediaCard";
import { TrackRow } from "@/components/TrackRow";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { getStrings } from "@/lib/i18n";
import { useSonataStore } from "@/store/useSonataStore";

export function SearchView() {
  const snapshot = useSonataStore((s) => s.snapshot);
  const dispatch = useSonataStore((s) => s.dispatch);
  const [query, setQuery] = useState(snapshot.search_query);
  const t = getStrings(snapshot.config.language);

  const runSearch = (q: string) => {
    const trimmed = q.trim();
    if (!trimmed) return;
    setQuery(trimmed);
    void dispatch({ type: "Search", query: trimmed });
  };

  const songs =
    snapshot.search_rich.songs.length > 0
      ? snapshot.search_rich.songs
      : snapshot.search_results;
  const topTrack = snapshot.search_rich.top_track || songs[0];
  const topCard = snapshot.search_rich.top_result;

  return (
    <div className="sonata-scroll h-full overflow-y-auto px-5 @md:px-8 pt-5 pb-36 flex flex-col gap-6">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          runSearch(query);
        }}
        className="flex items-center gap-2.5"
      >
        <div className="relative flex-1 max-w-xl">
          <Search className="pointer-events-none absolute left-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-[var(--ink-muted)]" />
          <input
            type="text"
            autoFocus
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              if (e.target.value.trim().length > 1) {
                void dispatch({ type: "FetchSearchSuggestions", query: e.target.value });
              }
            }}
            placeholder={t.searchPlaceholder}
            className="h-10 w-full rounded-full border border-[var(--border-hairline)] bg-[var(--bg-elevated)] pl-10 pr-4 text-sm text-[var(--ink-primary)] placeholder:text-[var(--ink-muted)] focus:border-[var(--border-strong)] focus:outline-none"
          />
        </div>
        <Button type="submit" size="pill" variant="default">
          {t.search}
        </Button>
      </form>

      {/* Suggestions & Recent Searches */}
      {!snapshot.search_loading &&
        songs.length === 0 &&
        (snapshot.search_suggestions.length > 0 || snapshot.recent_searches.length > 0) && (
          <div className="space-y-4">
            {snapshot.search_suggestions.length > 0 && (
              <div className="flex flex-wrap gap-2">
                {snapshot.search_suggestions.map((sug) => (
                  <button
                    key={sug}
                    type="button"
                    onClick={() => runSearch(sug)}
                    className="rounded-full border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-3 py-1.5 text-xs text-[var(--ink-secondary)] hover:text-[var(--ink-primary)]"
                  >
                    {sug}
                  </button>
                ))}
              </div>
            )}
            {snapshot.recent_searches.length > 0 && (
              <div>
                <h2 className="mb-2 text-xs font-semibold uppercase tracking-wider text-[var(--ink-muted)]">
                  {t.recentSearches}
                </h2>
                <div className="flex flex-wrap gap-2">
                  {snapshot.recent_searches.map((recent) => (
                    <button
                      key={recent}
                      type="button"
                      onClick={() => runSearch(recent)}
                      className="rounded-full border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-3 py-1.5 text-xs text-[var(--ink-secondary)] hover:text-[var(--ink-primary)]"
                    >
                      {recent}
                    </button>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}

      {snapshot.search_loading ? (
        <div className="py-12 text-sm text-[var(--ink-muted)]">{t.searching}</div>
      ) : (
        (songs.length > 0 ||
          topCard ||
          snapshot.search_rich.albums.length > 0 ||
          snapshot.search_rich.artists.length > 0) && (
          <div className="space-y-6">
            {/* Top Result Banner */}
            {(topCard || topTrack) && (
              <div className="grid grid-cols-1 @lg:grid-cols-[340px_1fr] gap-5">
                <div className="rounded-2xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] p-4 flex items-center gap-4">
                  <ArtworkImage
                    src={topCard?.thumbnail_url || topTrack?.thumbnail_url}
                    alt={topCard?.title || topTrack?.title || t.topResult}
                    className="h-20 w-20"
                    rounded={topCard?.is_artist ? "rounded-full" : "rounded-xl"}
                  />
                  <div className="min-w-0 flex-1">
                    <span className="text-[11px] font-semibold uppercase tracking-wider text-[var(--ink-muted)]">
                      {t.topResult}
                    </span>
                    <div className="mt-0.5 truncate text-lg font-bold text-[var(--ink-primary)]">
                      {topCard?.title || topTrack?.title}
                    </div>
                    <div className="truncate text-xs text-[var(--ink-secondary)]">
                      {topCard?.subtitle || topTrack?.artist}
                    </div>
                    {topTrack && (
                      <Button
                        size="sm"
                        variant="accent"
                        className="mt-2.5 h-7 rounded-full px-3 text-xs"
                        onClick={() =>
                          void dispatch({
                            type: "PlayTracks",
                            tracks: songs.length > 0 ? songs : [topTrack],
                            start_index: 0,
                            shuffle: false,
                          })
                        }
                      >
                        <Play className="h-3 w-3 fill-current" />
                        {t.play}
                      </Button>
                    )}
                  </div>
                </div>
              </div>
            )}

            <Tabs defaultValue="songs">
              <TabsList>
                <TabsTrigger value="songs">{t.songs} ({songs.length})</TabsTrigger>
                <TabsTrigger value="albums">
                  {t.albums} ({snapshot.search_rich.albums.length})
                </TabsTrigger>
                <TabsTrigger value="artists">
                  {t.artists} ({snapshot.search_rich.artists.length})
                </TabsTrigger>
                <TabsTrigger value="playlists">
                  {t.playlists} ({snapshot.search_rich.playlists.length})
                </TabsTrigger>
              </TabsList>

              <TabsContent value="songs">
                {songs.map((track, idx) => (
                  <TrackRow
                    key={`${track.id}-${idx}`}
                    track={track}
                    index={idx}
                    allTracks={songs}
                    showCover
                    showAlbum
                  />
                ))}
              </TabsContent>

              <TabsContent value="albums">
                <div className="grid grid-cols-[repeat(auto-fill,minmax(156px,1fr))] gap-3">
                  {snapshot.search_rich.albums.map((card) => (
                    <MediaCard key={card.id} card={card} layout="grid" />
                  ))}
                </div>
              </TabsContent>

              <TabsContent value="artists">
                <div className="grid grid-cols-[repeat(auto-fill,minmax(156px,1fr))] gap-3">
                  {snapshot.search_rich.artists.map((card) => (
                    <MediaCard key={card.id} card={card} layout="grid" />
                  ))}
                </div>
              </TabsContent>

              <TabsContent value="playlists">
                <div className="grid grid-cols-[repeat(auto-fill,minmax(156px,1fr))] gap-3">
                  {snapshot.search_rich.playlists.map((card) => (
                    <MediaCard key={card.id} card={card} layout="grid" />
                  ))}
                </div>
              </TabsContent>
            </Tabs>
          </div>
        )
      )}
    </div>
  );
}

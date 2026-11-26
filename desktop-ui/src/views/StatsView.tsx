import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { getStrings } from "@/lib/i18n";
import { formatDuration } from "@/lib/utils";
import { useSonataStore } from "@/store/useSonataStore";
import type { StatsWindow } from "@/types/sonata";

export function StatsView() {
  const snapshot = useSonataStore((s) => s.snapshot);
  const dispatch = useSonataStore((s) => s.dispatch);
  const t = getStrings(snapshot.config.language);
  const stats = snapshot.stats_summary;

  const hours = (stats.total_listening_secs / 3600).toFixed(1);

  return (
    <div className="sonata-scroll h-full overflow-y-auto px-5 @md:px-8 pt-5 pb-36 flex flex-col gap-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-3xl font-bold tracking-tight">{t.stats}</h1>
        <Tabs
          value={snapshot.stats_window}
          onValueChange={(w) => void dispatch({ type: "LoadListeningStats", window: w as StatsWindow })}
        >
          <TabsList>
            <TabsTrigger value="Week">{t.stats7Days}</TabsTrigger>
            <TabsTrigger value="Month">{t.stats30Days}</TabsTrigger>
            <TabsTrigger value="AllTime">{t.statsAllTime}</TabsTrigger>
          </TabsList>
        </Tabs>
      </div>

      <div className="grid grid-cols-2 @lg:grid-cols-4 gap-3">
        <div className="rounded-2xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] p-4">
          <div className="text-xs text-[var(--ink-muted)]">{t.totalPlays}</div>
          <div className="mt-1 text-2xl font-bold tabular-nums">{stats.total_plays}</div>
        </div>
        <div className="rounded-2xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] p-4">
          <div className="text-xs text-[var(--ink-muted)]">{t.listeningHours}</div>
          <div className="mt-1 text-2xl font-bold tabular-nums">{hours}{t.hoursUnit}</div>
        </div>
        <div className="rounded-2xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] p-4">
          <div className="text-xs text-[var(--ink-muted)]">{t.uniqueTracks}</div>
          <div className="mt-1 text-2xl font-bold tabular-nums">{stats.unique_tracks}</div>
        </div>
        <div className="rounded-2xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] p-4">
          <div className="text-xs text-[var(--ink-muted)]">{t.uniqueArtists}</div>
          <div className="mt-1 text-2xl font-bold tabular-nums">{stats.unique_artists}</div>
        </div>
      </div>

      <div className="grid grid-cols-1 @lg:grid-cols-2 gap-6">
        <section className="rounded-2xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] p-4 space-y-2.5">
          <h2 className="text-sm font-semibold">{t.topTracks}</h2>
          {stats.top_tracks.length === 0 ? (
            <p className="text-xs text-[var(--ink-muted)]">{t.noTrackPlays}</p>
          ) : (
            stats.top_tracks.map((row, idx) => (
              <div
                key={`${row.title}-${idx}`}
                className="flex items-center justify-between border-b border-[var(--border-hairline)] py-2 last:border-b-0"
              >
                <div className="min-w-0">
                  <div className="truncate text-xs font-semibold">{row.title}</div>
                  <div className="truncate text-[11px] text-[var(--ink-secondary)]">{row.artist}</div>
                </div>
                <div className="text-right text-[11px] text-[var(--ink-muted)] tabular-nums shrink-0 ml-3">
                  <div>{row.play_count} {t.playsLabel}</div>
                  <div>{formatDuration(row.total_secs)}</div>
                </div>
              </div>
            ))
          )}
        </section>

        <section className="rounded-2xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] p-4 space-y-2.5">
          <h2 className="text-sm font-semibold">{t.topArtists}</h2>
          {stats.top_artists.length === 0 ? (
            <p className="text-xs text-[var(--ink-muted)]">{t.noArtistPlays}</p>
          ) : (
            stats.top_artists.map((row, idx) => (
              <div
                key={`${row.artist}-${idx}`}
                className="flex items-center justify-between border-b border-[var(--border-hairline)] py-2 last:border-b-0"
              >
                <div className="truncate text-xs font-semibold">{row.artist}</div>
                <div className="text-right text-[11px] text-[var(--ink-muted)] tabular-nums shrink-0 ml-3">
                  <span>{row.play_count} {t.playsLabel}</span>
                </div>
              </div>
            ))
          )}
        </section>
      </div>
    </div>
  );
}

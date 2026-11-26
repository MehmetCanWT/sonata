import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { App } from "@/App";
import { PlayerDock } from "@/components/PlayerDock";
import { ArtistView } from "@/views/ArtistView";
import { ImmersiveLyricsView } from "@/views/ImmersiveLyricsView";
import { getStrings } from "@/i18n";
import { createEmptySnapshot } from "@/lib/ipc";
import {
  buildKaraokeTokens,
  computeTokenProgress,
  isLyricLineActive,
  resolveActiveLineIndex,
} from "@/lib/lyricsSync";
import { sanitizeAccentRgb } from "@/lib/utils";
import {
  applySnapshotNavTransition,
  HOME_HISTORY_ENTRY,
  useSonataStore,
} from "@/store/useSonataStore";

beforeAll(() => {
  if (typeof globalThis.ResizeObserver === "undefined") {
    globalThis.ResizeObserver = class ResizeObserver {
      observe() {}
      unobserve() {}
      disconnect() {}
    };
  }
  if (typeof globalThis.Element !== "undefined" && !Element.prototype.scrollIntoView) {
    Element.prototype.scrollIntoView = () => {};
  }
});

afterEach(() => {
  cleanup();
});

describe("Sonata Desktop UI", () => {
  it("renders 3-pane matte shell with sidebar, center viewport, player dock, and queue panel", async () => {
    render(<App />);
    expect(await screen.findByLabelText("Sidebar navigation")).toBeTruthy();
    expect(screen.getByLabelText("Playback Queue")).toBeTruthy();
    expect(screen.getByLabelText("Playback position")).toBeTruthy();
    expect(screen.getByText("Playing Next")).toBeTruthy();
  });

  it("filters violet/lavender hues out of extracted artwork accents", () => {
    const safe = sanitizeAccentRgb([180, 120, 235]);
    expect(safe).not.toEqual([180, 120, 235]);
  });

  it("handles Back and Forward navigation without re-pushing stale snapshots", async () => {
    useSonataStore.setState({
      currentEntry: HOME_HISTORY_ENTRY,
      navBack: [],
      navForward: [],
      pendingNavKey: null,
      librarySection: "albums",
    });

    const baseSnap = createEmptySnapshot();
    const albumSnap = {
      ...baseSnap,
      active_nav: { kind: "Album" as const, id: "album-1" },
      album_detail: {
        id: "album-1",
        title: "Test Album",
        artist: "Test Artist",
        tracks: [],
        is_saved: false,
      },
    };
    const artistSnap = {
      ...baseSnap,
      active_nav: { kind: "Artist" as const, id: "artist-1" },
      artist_page: {
        id: "artist-1",
        name: "Test Artist",
        top_tracks: [],
        albums: [],
        singles: [],
        playlists: [],
        related_artists: [],
      },
    };

    // Simulate backend opening Album from Home, then Artist from Album
    useSonataStore.setState((s) => ({
      snapshot: albumSnap,
      ...applySnapshotNavTransition(s, albumSnap),
    }));
    useSonataStore.setState((s) => ({
      snapshot: artistSnap,
      ...applySnapshotNavTransition(s, artistSnap),
    }));

    expect(useSonataStore.getState().navBack.map((e) => e.key)).toEqual(["Home", "Album:album-1"]);
    expect(useSonataStore.getState().currentEntry.key).toBe("Artist:artist-1");

    // Trigger goBack -> moves current Artist onto navForward and expects Album:album-1
    await useSonataStore.getState().goBack();
    expect(useSonataStore.getState().pendingNavKey).toBe("Album:album-1");

    // Stale Artist snapshot arriving before Album settles must NOT corrupt navBack/navForward
    useSonataStore.setState((s) => applySnapshotNavTransition(s, artistSnap));
    expect(useSonataStore.getState().navBack.map((e) => e.key)).toEqual(["Home"]);
    expect(useSonataStore.getState().navForward.map((e) => e.key)).toEqual(["Artist:artist-1"]);

    // Settled Album snapshot arrives -> clears pendingNavKey and keeps stacks clean
    useSonataStore.setState((s) => ({
      snapshot: albumSnap,
      ...applySnapshotNavTransition(s, albumSnap),
    }));
    expect(useSonataStore.getState().pendingNavKey).toBeNull();
    expect(useSonataStore.getState().navBack.map((e) => e.key)).toEqual(["Home"]);
    expect(useSonataStore.getState().navForward.map((e) => e.key)).toEqual(["Artist:artist-1"]);

    // goForward restores Artist cleanly
    await useSonataStore.getState().goForward();
    useSonataStore.setState((s) => ({
      snapshot: artistSnap,
      ...applySnapshotNavTransition(s, artistSnap),
    }));
    expect(useSonataStore.getState().currentEntry.key).toBe("Artist:artist-1");
    expect(useSonataStore.getState().navBack.map((e) => e.key)).toEqual(["Home", "Album:album-1"]);
    expect(useSonataStore.getState().navForward).toEqual([]);
  });

  it("resolves lyric lines and token sweep progress accurately for word- and line-synced verses", () => {
    const lines = [
      {
        text: "first line here",
        start_ms: 1000,
        end_ms: 2800,
        words: [
          { text: "first", start_ms: 1000, end_ms: 1400 },
          { text: "line", start_ms: 1500, end_ms: 2000 },
          { text: "here", start_ms: 2100, end_ms: 2800 },
        ],
        background: false,
      },
      {
        text: "plain second verse",
        start_ms: 3500,
        end_ms: 6000,
        words: [],
        background: false,
      },
    ];

    expect(resolveActiveLineIndex(lines, 500)).toBe(-1);
    expect(resolveActiveLineIndex(lines, 970)).toBe(0);
    expect(resolveActiveLineIndex(lines, 3480)).toBe(1);

    const wordTokens = buildKaraokeTokens(lines[0], lines[1].start_ms);
    expect(wordTokens).toHaveLength(3);
    expect(computeTokenProgress(900, wordTokens[0].startMs, wordTokens[0].endMs)).toBe(0);
    const mid = computeTokenProgress(1165, wordTokens[0].startMs, wordTokens[0].endMs);
    expect(mid).toBeGreaterThan(0.45);
    expect(mid).toBeLessThan(0.55);
    expect(computeTokenProgress(1450, wordTokens[0].startMs, wordTokens[0].endMs)).toBe(1);

    // Line-synced verse distributes smooth monotonic tokens across its words.
    const synthTokens = buildKaraokeTokens(lines[1], 6500);
    expect(synthTokens.map((t) => t.text)).toEqual(["plain", "second", "verse"]);
    expect(synthTokens[0].startMs).toBe(3500);
    expect(synthTokens[1].startMs).toBeGreaterThan(synthTokens[0].startMs);
    expect(synthTokens[2].endMs).toBeGreaterThan(synthTokens[1].endMs);
  });

  it("renders a single Lyrics toggle in PlayerDock without a duplicate Fullscreen button", () => {
    useSonataStore.setState({ snapshot: createEmptySnapshot() });
    render(<PlayerDock />);
    expect(screen.getByRole("button", { name: "Lyrics" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Fullscreen" })).toBeNull();
  });

  it("pauses lyrics auto-alignment on manual scroll and re-syncs via the top Sync button", () => {
    const scrollIntoViewMock = vi.fn();
    Element.prototype.scrollIntoView = scrollIntoViewMock;

    const baseSnap = createEmptySnapshot();
    const syncedSnap = {
      ...baseSnap,
      paused: true,
      position_secs: 1.5,
      duration_secs: 180,
      queue: [
        {
          id: "test-track-1",
          title: "Test Track",
          artist: "Test Artist",
          album: "Test Album",
          duration_secs: 180,
          source: "YouTubeMusic" as const,
          liked: false,
          is_explicit: false,
        },
      ],
      current_index: 0,
      lyrics: {
        kind: "Synced" as const,
        lines: [
          {
            text: "first test verse",
            start_ms: 1000,
            end_ms: 3000,
            words: [],
            background: false,
          },
          {
            text: "second test verse",
            start_ms: 4000,
            end_ms: 6000,
            words: [],
            background: false,
          },
        ],
      },
    };
    useSonataStore.setState({
      snapshot: syncedSnap,
      progress: {
        position_secs: 1.5,
        duration_secs: 180,
        paused: true,
        playback_speed: 1,
        loading_track: false,
        syncedAtMs: performance.now(),
      },
      immersiveOpen: true,
      immersiveTab: "lyrics",
    });

    const { container } = render(<ImmersiveLyricsView />);
    expect(screen.queryByRole("button", { name: "Sync" })).toBeNull();
    const initialCalls = scrollIntoViewMock.mock.calls.length;

    const scroller = container.querySelector(".overflow-y-auto");
    expect(scroller).toBeTruthy();
    if (scroller) {
      // Passive layout scroll, clicks, horizontal wheel, and tiny touch jitter must NOT break sync.
      fireEvent.scroll(scroller);
      fireEvent.click(scroller);
      fireEvent.wheel(scroller, { deltaY: 0, deltaX: 50 });
      fireEvent.touchStart(scroller, { touches: [{ clientY: 200 }] });
      fireEvent.touchMove(scroller, { touches: [{ clientY: 204 }] });
      fireEvent.touchEnd(scroller);
      expect(screen.queryByRole("button", { name: "Sync" })).toBeNull();

      // Genuine vertical scroll pauses autoFollow and shows Sync.
      fireEvent.wheel(scroller, { deltaY: 120 });
    }

    const syncBtn = screen.getByRole("button", { name: "Sync" });
    expect(syncBtn).toBeTruthy();

    fireEvent.click(syncBtn);
    expect(screen.queryByRole("button", { name: "Sync" })).toBeNull();
    expect(scrollIntoViewMock.mock.calls.length).toBeGreaterThan(initialCalls);

    if (scroller) {
      // Genuine vertical touch drag (>= 12px) also pauses autoFollow and shows Sync.
      fireEvent.touchStart(scroller, { touches: [{ clientY: 200 }] });
      fireEvent.touchMove(scroller, { touches: [{ clientY: 240 }] });
      fireEvent.touchEnd(scroller);
      expect(screen.getByRole("button", { name: "Sync" })).toBeTruthy();
    }
  });

  it("keeps lead verse, overlapping parenthesized background vocal, and counter duet lines active simultaneously", () => {
    const duetAndBgLines = [
      {
        text: "Main lead verse still playing",
        start_ms: 1000,
        end_ms: 5200,
        words: [],
        speaker: "Lead",
        background: false,
      },
      {
        text: "(backing ad-lib)",
        start_ms: 2400,
        end_ms: 3900,
        words: [],
        speaker: "Lead",
        background: true,
      },
      {
        text: "Counter duet overlap line",
        start_ms: 2600,
        end_ms: 4800,
        words: [],
        speaker: "Counter",
        background: false,
      },
      {
        text: "Following verse",
        start_ms: 5500,
        end_ms: 8000,
        words: [],
        speaker: "Lead",
        background: false,
      },
    ];

    const anchor = resolveActiveLineIndex(duetAndBgLines, 2700);
    // Anchor stays on lead or counter without losing simultaneous lead/bg activity
    expect(anchor).toBeGreaterThanOrEqual(0);
    expect(isLyricLineActive(duetAndBgLines, 0, anchor, 2700)).toBe(true);
    expect(isLyricLineActive(duetAndBgLines, 1, anchor, 2700)).toBe(true);
    expect(isLyricLineActive(duetAndBgLines, 2, anchor, 2700)).toBe(true);
    expect(isLyricLineActive(duetAndBgLines, 3, anchor, 2700)).toBe(false);

    const overlapAnchor = resolveActiveLineIndex(duetAndBgLines.slice(0, 2), 2600);
    expect(overlapAnchor).toBe(0);
  });

  it("provides complete non-empty i18n dictionaries for English, Turkish, and Russian", () => {
    const en = getStrings("en");
    const tr = getStrings("tr");
    const ru = getStrings("ru");
    const enKeys = Object.keys(en).sort();
    expect(Object.keys(tr).sort()).toEqual(enKeys);
    expect(Object.keys(ru).sort()).toEqual(enKeys);
    for (const key of enKeys) {
      const k = key as keyof typeof en;
      expect(en[k].trim().length).toBeGreaterThan(0);
      expect(tr[k].trim().length).toBeGreaterThan(0);
      expect(ru[k].trim().length).toBeGreaterThan(0);
    }
    expect(tr.home).not.toBe(en.home);
    expect(ru.home).not.toBe(en.home);
  });

  it("renders library artist hero photo, full-width TrackRow list, and download action", () => {
    const baseSnap = createEmptySnapshot();
    useSonataStore.setState({
      snapshot: {
        ...baseSnap,
        artist_page: {
          id: "MPLAUC_test_artist",
          name: "Test Artist",
          thumbnail_url: "https://img.example/artist.jpg",
          banner_url: null,
          top_tracks: [
            {
              id: "test-track-1",
              title: "Full Width Library Song Title",
              artist: "Test Artist",
              album: "Test Album",
              duration_secs: 210,
              thumbnail_url: "https://img.example/song.jpg",
              source: "YouTubeMusic",
              liked: false,
              is_explicit: false,
            },
          ],
          albums: [],
          singles: [],
          playlists: [],
          related_artists: [],
        },
      },
    });

    const { container } = render(<ArtistView />);
    const heroImg = screen.getByRole("img", { name: "Test Artist" }) as HTMLImageElement;
    expect(heroImg.src).toContain("https://img.example/artist.jpg");
    expect(screen.getByText("Full Width Library Song Title")).toBeTruthy();
    expect(screen.getByText("Test Album")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Download All" })).toBeTruthy();
    expect(container.innerHTML).not.toContain("grid-cols-[248px_1fr]");
  });
});

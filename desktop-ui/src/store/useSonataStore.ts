import { useEffect, useRef, useState } from "react";
import { create } from "zustand";
import {
  createEmptySnapshot,
  dispatchBackendCmd,
  fetchDesktopSettings,
  fetchPlatform,
  fetchShowWindowControls,
  saveDesktopSettings,
  subscribeBackendState,
} from "@/lib/ipc";
import type {
  BackendCmdDto,
  DesktopSettingsDto,
  NavTargetDto,
  PlatformKind,
  ProgressTickDto,
  ServerProfile,
  ThemeModePref,
  Track,
  UiSnapshotDto,
} from "@/types/sonata";

export type LibrarySection =
  | "pins"
  | "recent"
  | "songs"
  | "albums"
  | "artists"
  | "downloads"
  | "local_servers"
  | "all_playlists";

export type FullscreenTab = "cover" | "lyrics" | "queue";

export type ActiveModal =
  | null
  | { type: "login" }
  | { type: "createPlaylist"; kind: "ytm" | "personal" }
  | { type: "addToPlaylist"; track: Track }
  | { type: "editPlaylist"; playlistId: string; currentTitle: string }
  | { type: "sleepTimer" }
  | { type: "serverEditor"; profile?: ServerProfile };

export interface NavHistoryEntry {
  key: string;
  cmd: BackendCmdDto;
  librarySection?: LibrarySection;
}

const MAX_NAV_HISTORY = 20;

export const HOME_HISTORY_ENTRY: NavHistoryEntry = {
  key: "Home",
  cmd: { type: "Navigate", target: { kind: "Home" } },
};

export function entryForTarget(
  target: NavTargetDto,
  librarySection: LibrarySection,
): NavHistoryEntry {
  switch (target.kind) {
    case "Library":
      return {
        key: `Library:${librarySection}`,
        cmd: { type: "Navigate", target: { kind: "Library" } },
        librarySection,
      };
    case "Album":
      return {
        key: `Album:${target.id}`,
        cmd: {
          type: "OpenAlbum",
          album_id: target.id || null,
          title: "",
          artist: "",
          thumbnail_url: null,
        },
      };
    case "Artist":
      return {
        key: `Artist:${target.id}`,
        cmd: target.id.startsWith("name:")
          ? { type: "OpenArtistByName", name: target.id.slice(5) }
          : { type: "OpenArtist", id: target.id },
      };
    case "Playlist":
      return {
        key: `Playlist:${target.id}`,
        cmd: {
          type: "OpenCollection",
          id: target.id,
          title: "",
          subtitle: "",
          thumbnail_url: null,
          editable: false,
        },
      };
    case "MoodCategory":
      return {
        key: `MoodCategory:${target.browse_id}:${target.params}`,
        cmd: {
          type: "OpenMoodCategory",
          title: target.title,
          browse_id: target.browse_id,
          params: target.params,
        },
      };
    case "ShelfDetail":
      return {
        key: `ShelfDetail:${target.title}`,
        cmd: { type: "Navigate", target },
      };
    default:
      return {
        key: target.kind,
        cmd: { type: "Navigate", target },
      };
  }
}

export function deriveHistoryEntry(
  snap: UiSnapshotDto,
  librarySection: LibrarySection,
): NavHistoryEntry {
  const nav = snap.active_nav;
  switch (nav.kind) {
    case "Library":
      return {
        key: `Library:${librarySection}`,
        cmd: { type: "Navigate", target: { kind: "Library" } },
        librarySection,
      };
    case "Album": {
      const id = nav.id || snap.album_detail?.id || "";
      return {
        key: `Album:${id}`,
        cmd: {
          type: "OpenAlbum",
          album_id: snap.album_detail?.id || (id || null),
          title: snap.album_detail?.title || "",
          artist: snap.album_detail?.artist || "",
          thumbnail_url: snap.album_detail?.thumbnail_url ?? null,
        },
      };
    }
    case "Artist": {
      const resolvedId = snap.artist_page?.id;
      const keyId = nav.id || resolvedId || "";
      return {
        key: `Artist:${keyId}`,
        cmd: resolvedId
          ? { type: "OpenArtist", id: resolvedId }
          : keyId.startsWith("name:")
            ? { type: "OpenArtistByName", name: keyId.slice(5) }
            : { type: "OpenArtist", id: keyId },
      };
    }
    case "Playlist": {
      const id = snap.playlist_id || nav.id;
      return {
        key: `Playlist:${nav.id}`,
        cmd: {
          type: "OpenCollection",
          id,
          title: snap.playlist_heading,
          subtitle: snap.playlist_subtitle,
          thumbnail_url: snap.playlist_cover_url ?? null,
          editable: snap.playlist_editable,
        },
      };
    }
    case "MoodCategory":
      return {
        key: `MoodCategory:${nav.browse_id}:${nav.params}`,
        cmd: {
          type: "OpenMoodCategory",
          title: nav.title,
          browse_id: nav.browse_id,
          params: nav.params,
        },
      };
    case "ShelfDetail":
      return {
        key: `ShelfDetail:${nav.title}`,
        cmd: snap.shelf_detail
          ? { type: "OpenShelfDetail", shelf: snap.shelf_detail }
          : { type: "Navigate", target: nav },
      };
    default:
      return {
        key: nav.kind,
        cmd: { type: "Navigate", target: nav },
      };
  }
}

export interface NavTransitionState {
  currentEntry: NavHistoryEntry;
  navBack: NavHistoryEntry[];
  navForward: NavHistoryEntry[];
  pendingNavKey: string | null;
  librarySection: LibrarySection;
}

export function applySnapshotNavTransition(
  state: NavTransitionState,
  snap: UiSnapshotDto,
): Pick<
  NavTransitionState,
  "currentEntry" | "navBack" | "navForward" | "pendingNavKey"
> {
  const incoming = deriveHistoryEntry(snap, state.librarySection);

  if (state.pendingNavKey !== null) {
    // Only settle pending Back/Forward/Navigate transitions once the backend
    // confirms the target view, ignoring stale snapshots in flight.
    if (incoming.key === state.pendingNavKey) {
      return {
        currentEntry: incoming,
        navBack: state.navBack,
        navForward: state.navForward,
        pendingNavKey: null,
      };
    }
    return {
      currentEntry: state.currentEntry,
      navBack: state.navBack,
      navForward: state.navForward,
      pendingNavKey: state.pendingNavKey,
    };
  }

  if (incoming.key !== state.currentEntry.key) {
    const lastBack = state.navBack[state.navBack.length - 1];
    const nextBack =
      lastBack?.key === state.currentEntry.key
        ? state.navBack
        : [...state.navBack.slice(-(MAX_NAV_HISTORY - 1)), state.currentEntry];
    return {
      currentEntry: incoming,
      navBack: nextBack,
      navForward: [],
      pendingNavKey: null,
    };
  }

  return {
    currentEntry: incoming,
    navBack: state.navBack,
    navForward: state.navForward,
    pendingNavKey: null,
  };
}

interface SonataStore {
  snapshot: UiSnapshotDto;
  progress: ProgressTickDto & { syncedAtMs: number };
  settings: DesktopSettingsDto;
  platform: PlatformKind;
  showWindowControls: boolean;
  sidebarOpen: boolean;
  queueOpen: boolean;
  immersiveOpen: boolean;
  immersiveTab: FullscreenTab;
  librarySection: LibrarySection;
  activeModal: ActiveModal;
  currentEntry: NavHistoryEntry;
  navBack: NavHistoryEntry[];
  navForward: NavHistoryEntry[];
  pendingNavKey: string | null;

  init: () => Promise<() => void>;
  dispatch: (cmd: BackendCmdDto) => Promise<void>;
  navigate: (target: NavTargetDto, librarySection?: LibrarySection) => Promise<void>;
  goBack: () => Promise<void>;
  goForward: () => Promise<void>;
  toggleSidebar: () => void;
  setSidebarOpen: (open: boolean) => void;
  toggleQueue: () => void;
  setImmersiveOpen: (open: boolean) => void;
  setImmersiveTab: (tab: FullscreenTab) => void;
  openImmersive: (tab?: FullscreenTab) => void;
  setModal: (modal: ActiveModal) => void;
  setLibrarySection: (section: LibrarySection) => void;
  updateSettings: (patch: Partial<DesktopSettingsDto>) => Promise<void>;
  setThemeMode: (mode: ThemeModePref) => Promise<void>;
  togglePinCollection: (item: {
    id: string;
    title: string;
    subtitle: string;
    thumbnail_url?: string | null;
    is_album: boolean;
  }) => Promise<void>;
  setVolumeOptimistic: (vol: number) => void;
}

function applyThemeToDocument(mode: ThemeModePref, reduceMotion: boolean) {
  if (typeof document === "undefined") return;
  const prefersDark =
    typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-color-scheme: dark)")?.matches;
  const resolvedDark = mode === "dark" || (mode === "system" && prefersDark);
  const root = document.documentElement;
  root.classList.toggle("dark", resolvedDark);
  root.classList.toggle("light", !resolvedDark);
  root.setAttribute("data-reduce-motion", reduceMotion ? "true" : "false");
}

const initialSnap = createEmptySnapshot();

export const useSonataStore = create<SonataStore>((set, get) => ({
  snapshot: initialSnap,
  progress: {
    position_secs: initialSnap.position_secs,
    duration_secs: initialSnap.duration_secs,
    paused: initialSnap.paused,
    loading_track: initialSnap.loading_track,
    playback_speed: initialSnap.playback_speed,
    syncedAtMs: performance.now(),
  },
  settings: {
    language: "en",
    theme_mode: "dark",
    adaptive_art_color: true,
    show_queue_panel: true,
    show_lyrics_panel: false,
    double_click_to_play: false,
    pinned_collections: [],
  },
  platform: "linux",
  showWindowControls: false,
  sidebarOpen: true,
  queueOpen: true,
  immersiveOpen: false,
  immersiveTab: "lyrics",
  librarySection: "albums",
  activeModal: null,
  currentEntry: HOME_HISTORY_ENTRY,
  navBack: [],
  navForward: [],
  pendingNavKey: null,

  init: async () => {
    const [platform, showWindowControls, settings] = await Promise.all([
      fetchPlatform(),
      fetchShowWindowControls(),
      fetchDesktopSettings(),
    ]);
    set((s) => ({
      platform,
      showWindowControls,
      settings,
      queueOpen: settings.show_queue_panel,
      snapshot: s.snapshot,
    }));
    applyThemeToDocument(settings.theme_mode, get().snapshot.config.reduce_motion);

    const unsub = await subscribeBackendState((ev) => {
      if (ev.type === "Snapshot") {
        set((state) => {
          const navUpdate = applySnapshotNavTransition(state, ev.data);
          applyThemeToDocument(state.settings.theme_mode, ev.data.config.reduce_motion);
          return {
            snapshot: ev.data,
            ...navUpdate,
            progress: {
              position_secs: ev.data.position_secs,
              duration_secs: ev.data.duration_secs,
              paused: ev.data.paused,
              loading_track: ev.data.loading_track,
              playback_speed: ev.data.playback_speed,
              syncedAtMs: performance.now(),
            },
          };
        });
      } else {
        set({
          progress: {
            ...ev.data,
            syncedAtMs: performance.now(),
          },
        });
      }
    });

    return unsub;
  },

  dispatch: async (cmd) => {
    if (
      cmd.type === "OpenAlbum" ||
      cmd.type === "OpenArtist" ||
      cmd.type === "OpenArtistByName" ||
      cmd.type === "OpenCollection" ||
      cmd.type === "OpenMoodCategory" ||
      cmd.type === "OpenShelfDetail"
    ) {
      set({ pendingNavKey: null });
    }
    await dispatchBackendCmd(cmd);
  },

  navigate: async (target, librarySection) => {
    const state = get();
    const nextLibSection = librarySection ?? state.librarySection;
    const nextEntry = entryForTarget(target, nextLibSection);
    const shouldPush = nextEntry.key !== state.currentEntry.key;

    set((s) => ({
      librarySection: nextLibSection,
      currentEntry: nextEntry,
      navBack: shouldPush
        ? [...s.navBack.slice(-(MAX_NAV_HISTORY - 1)), s.currentEntry]
        : s.navBack,
      navForward: shouldPush ? [] : s.navForward,
      pendingNavKey: nextEntry.key,
    }));
    await dispatchBackendCmd(nextEntry.cmd);
  },

  goBack: async () => {
    const state = get();
    if (state.navBack.length === 0) return;
    const prev = state.navBack[state.navBack.length - 1];
    const current = state.currentEntry;
    set({
      navBack: state.navBack.slice(0, -1),
      navForward: [current, ...state.navForward.slice(0, MAX_NAV_HISTORY - 1)],
      currentEntry: prev,
      librarySection: prev.librarySection ?? state.librarySection,
      pendingNavKey: prev.key,
    });
    await dispatchBackendCmd(prev.cmd);
  },

  goForward: async () => {
    const state = get();
    if (state.navForward.length === 0) return;
    const next = state.navForward[0];
    const current = state.currentEntry;
    set({
      navBack: [...state.navBack.slice(-(MAX_NAV_HISTORY - 1)), current],
      navForward: state.navForward.slice(1),
      currentEntry: next,
      librarySection: next.librarySection ?? state.librarySection,
      pendingNavKey: next.key,
    });
    await dispatchBackendCmd(next.cmd);
  },

  toggleSidebar: () => set((s) => ({ sidebarOpen: !s.sidebarOpen })),
  setSidebarOpen: (open) => set({ sidebarOpen: open }),

  toggleQueue: () => {
    const nextOpen = !get().queueOpen;
    set({ queueOpen: nextOpen });
    void get().updateSettings({ show_queue_panel: nextOpen });
  },

  setImmersiveOpen: (open) => set({ immersiveOpen: open }),
  setImmersiveTab: (tab) => set({ immersiveTab: tab }),
  openImmersive: (tab) =>
    set((s) => ({
      immersiveOpen: true,
      immersiveTab: tab ?? s.immersiveTab,
    })),
  setModal: (modal) => set({ activeModal: modal }),
  setLibrarySection: (section) => set({ librarySection: section }),

  updateSettings: async (patch) => {
    const merged = { ...get().settings, ...patch };
    set({ settings: merged });
    applyThemeToDocument(merged.theme_mode, get().snapshot.config.reduce_motion);
    await saveDesktopSettings(merged);
  },

  setThemeMode: async (mode) => {
    await get().updateSettings({ theme_mode: mode });
  },

  togglePinCollection: async (item) => {
    const existing = get().settings.pinned_collections;
    const exists = existing.some((p) => p.id === item.id);
    const nextPinned = exists
      ? existing.filter((p) => p.id !== item.id)
      : [item, ...existing];
    await get().updateSettings({ pinned_collections: nextPinned });
  },

  setVolumeOptimistic: (vol) => {
    const clamped = Math.max(0, Math.min(100, Math.round(vol)));
    set((s) => ({
      snapshot: { ...s.snapshot, volume: clamped },
    }));
    void dispatchBackendCmd({ type: "SetVolume", volume: clamped });
  },
}));

export interface InterpolatedPositionOptions {
  intervalMs?: number;
  enabled?: boolean;
}

function computeInterpolatedSeconds(
  progress: ProgressTickDto & { syncedAtMs: number },
): number {
  if (progress.paused || progress.loading_track) {
    return progress.position_secs;
  }
  const elapsedSecs =
    ((performance.now() - progress.syncedAtMs) / 1000) *
    (progress.playback_speed || 1);
  return progress.duration_secs > 0
    ? Math.min(progress.duration_secs, progress.position_secs + elapsedSecs)
    : progress.position_secs + elapsedSecs;
}

/**
 * Interpolated playback clock synchronized from Rust `ProgressTickDto` events.
 * Uses 60fps `requestAnimationFrame` by default (`intervalMs = 0`) or throttled `setInterval`,
 * and suspends ticking whenever the window/document is hidden.
 */
export function useInterpolatedPosition(options?: InterpolatedPositionOptions): number {
  const intervalMs = options?.intervalMs ?? 0;
  const enabled = options?.enabled ?? true;
  const progress = useSonataStore((s) => s.progress);
  const progressRef = useRef(progress);
  progressRef.current = progress;

  const [pos, setPos] = useState(() => computeInterpolatedSeconds(progress));

  useEffect(() => {
    if (!enabled) {
      return;
    }
    const next = computeInterpolatedSeconds(progress);
    setPos((prev) => {
      if (intervalMs >= 1000 && !progress.paused && !progress.loading_track) {
        return Math.abs(next - prev) >= 1.5 ? next : prev;
      }
      return next;
    });
  }, [enabled, intervalMs, progress]);

  useEffect(() => {
    if (!enabled || progress.paused || progress.loading_track) {
      return;
    }

    let rafId = 0;
    let timerId = 0;
    let active = true;

    const stopLoop = () => {
      if (rafId) {
        cancelAnimationFrame(rafId);
        rafId = 0;
      }
      if (timerId) {
        window.clearInterval(timerId);
        timerId = 0;
      }
    };

    const tickRaf = () => {
      if (!active) return;
      setPos(computeInterpolatedSeconds(progressRef.current));
      rafId = requestAnimationFrame(tickRaf);
    };

    const startLoop = () => {
      stopLoop();
      if (
        !active ||
        (typeof document !== "undefined" && document.visibilityState === "hidden")
      ) {
        return;
      }
      if (intervalMs > 0) {
        timerId = window.setInterval(() => {
          setPos(computeInterpolatedSeconds(progressRef.current));
        }, intervalMs);
      } else {
        rafId = requestAnimationFrame(tickRaf);
      }
    };

    const onVisibilityChange = () => {
      if (typeof document !== "undefined" && document.visibilityState === "hidden") {
        stopLoop();
      } else {
        setPos(computeInterpolatedSeconds(progressRef.current));
        startLoop();
      }
    };

    startLoop();
    if (typeof document !== "undefined") {
      document.addEventListener("visibilitychange", onVisibilityChange);
    }

    return () => {
      active = false;
      stopLoop();
      if (typeof document !== "undefined") {
        document.removeEventListener("visibilitychange", onVisibilityChange);
      }
    };
  }, [enabled, intervalMs, progress.loading_track, progress.paused]);

  return pos;
}

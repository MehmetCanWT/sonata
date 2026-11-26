import {
  ChevronDown,
  ChevronRight,
  Clock,
  Compass,
  Disc3,
  Download,
  FolderOpen,
  Heart,
  Home,
  LayoutGrid,
  ListMusic,
  Mic2,
  Music2,
  Pin,
  Plus,
  Radio,
  Search,
  Settings,
  TrendingUp,
  User,
} from "lucide-react";
import { useState } from "react";
import { ArtworkImage } from "@/components/ArtworkImage";
import { getStrings } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { type LibrarySection, useSonataStore } from "@/store/useSonataStore";
import type { NavTargetDto } from "@/types/sonata";

function isActiveNav(active: NavTargetDto, kind: NavTargetDto["kind"]): boolean {
  return active.kind === kind;
}

export function Sidebar() {
  const snapshot = useSonataStore((s) => s.snapshot);
  const settings = useSonataStore((s) => s.settings);
  const platform = useSonataStore((s) => s.platform);
  const librarySection = useSonataStore((s) => s.librarySection);
  const navigate = useSonataStore((s) => s.navigate);
  const dispatch = useSonataStore((s) => s.dispatch);
  const setModal = useSonataStore((s) => s.setModal);
  const setLibrarySection = useSonataStore((s) => s.setLibrarySection);

  const [libExpanded, setLibExpanded] = useState(true);
  const [playlistsExpanded, setPlaylistsExpanded] = useState(true);
  const [searchDraft, setSearchDraft] = useState("");

  const t = getStrings(snapshot.config.language);

  const openLibrarySection = (section: LibrarySection) => {
    setLibrarySection(section);
    void navigate({ kind: "Library" }, section);
  };

  const submitSearch = (e: React.FormEvent) => {
    e.preventDefault();
    const q = searchDraft.trim();
    if (!q) {
      void navigate({ kind: "Search" });
      return;
    }
    void dispatch({ type: "Search", query: q });
    void navigate({ kind: "Search" });
  };

  const navItemClass = (active: boolean) =>
    cn(
      "group flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-[13px] font-medium transition-colors text-left",
      active
        ? "bg-[var(--bg-elevated)] text-[var(--ink-primary)]"
        : "text-[var(--ink-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--ink-primary)]",
    );

  return (
    <aside
      aria-label={t.sidebarNavigation}
      className={cn(
        "flex h-full w-[216px] shrink-0 flex-col bg-[var(--bg-sidebar)] select-none",
        platform === "macos" ? "pt-9" : "pt-3",
      )}
    >
      {/* Top draggable area + search pill */}
      <div className="px-3 pb-3" data-tauri-drag-region>
        <form onSubmit={submitSearch} className="relative">
          <Search className="pointer-events-none absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[var(--ink-muted)]" />
          <input
            type="text"
            value={searchDraft}
            onChange={(e) => {
              setSearchDraft(e.target.value);
              if (e.target.value.trim().length > 1) {
                void dispatch({ type: "FetchSearchSuggestions", query: e.target.value });
              }
            }}
            onFocus={() => {
              if (snapshot.active_nav.kind !== "Search") {
                void navigate({ kind: "Search" });
              }
            }}
            placeholder={t.search}
            className="h-8 w-full rounded-full border border-[var(--border-hairline)] bg-[var(--bg-elevated)] pl-8 pr-3 text-xs text-[var(--ink-primary)] placeholder:text-[var(--ink-muted)] focus:outline-none focus:border-[var(--border-strong)]"
          />
        </form>
      </div>

      {/* Main scrollable navigation */}
      <div className="sonata-scroll flex-1 overflow-y-auto px-2.5 space-y-4 pb-3">
        <div className="space-y-0.5">
          <button
            type="button"
            onClick={() => void navigate({ kind: "Home" })}
            className={navItemClass(isActiveNav(snapshot.active_nav, "Home"))}
          >
            <Home className="h-4 w-4 text-[var(--accent)]" />
            <span>{t.home}</span>
          </button>
          <button
            type="button"
            onClick={() => void navigate({ kind: "Explore" })}
            className={navItemClass(isActiveNav(snapshot.active_nav, "Explore"))}
          >
            <LayoutGrid className="h-4 w-4" />
            <span>{t.explore}</span>
          </button>
          <button
            type="button"
            onClick={() => void navigate({ kind: "Radio" })}
            className={navItemClass(isActiveNav(snapshot.active_nav, "Radio"))}
          >
            <Radio className="h-4 w-4" />
            <span>{t.radio}</span>
          </button>
          <button
            type="button"
            onClick={() => void navigate({ kind: "History" })}
            className={navItemClass(isActiveNav(snapshot.active_nav, "History"))}
          >
            <Clock className="h-4 w-4" />
            <span>{t.history}</span>
          </button>
          <button
            type="button"
            onClick={() => void navigate({ kind: "Stats" })}
            className={navItemClass(isActiveNav(snapshot.active_nav, "Stats"))}
          >
            <TrendingUp className="h-4 w-4" />
            <span>{t.stats}</span>
          </button>
        </div>

        {/* Library collapsible section */}
        <div>
          <button
            type="button"
            onClick={() => setLibExpanded((v) => !v)}
            className="flex w-full items-center justify-between px-2.5 py-1 text-[11px] font-semibold tracking-wide text-[var(--ink-muted)] hover:text-[var(--ink-secondary)]"
          >
            <span>{t.library}</span>
            {libExpanded ? (
              <ChevronDown className="h-3.5 w-3.5" />
            ) : (
              <ChevronRight className="h-3.5 w-3.5" />
            )}
          </button>
          {libExpanded && (
            <div className="mt-1 space-y-0.5">
              <button
                type="button"
                onClick={() => openLibrarySection("pins")}
                className={navItemClass(
                  snapshot.active_nav.kind === "Library" && librarySection === "pins",
                )}
              >
                <Pin className="h-3.5 w-3.5" />
                <span className="truncate">{t.pins}</span>
                {settings.pinned_collections.length > 0 && (
                  <span className="ml-auto text-[11px] text-[var(--ink-muted)] tabular-nums">
                    {settings.pinned_collections.length}
                  </span>
                )}
              </button>
              <button
                type="button"
                onClick={() => openLibrarySection("recent")}
                className={navItemClass(
                  snapshot.active_nav.kind === "Library" && librarySection === "recent",
                )}
              >
                <Compass className="h-3.5 w-3.5" />
                <span className="truncate">{t.recentlyAdded}</span>
              </button>
              <button
                type="button"
                onClick={() => openLibrarySection("songs")}
                className={navItemClass(
                  snapshot.active_nav.kind === "Library" && librarySection === "songs",
                )}
              >
                <Music2 className="h-3.5 w-3.5" />
                <span className="truncate">{t.songs}</span>
              </button>
              <button
                type="button"
                onClick={() => openLibrarySection("albums")}
                className={navItemClass(
                  snapshot.active_nav.kind === "Library" && librarySection === "albums",
                )}
              >
                <Disc3 className="h-3.5 w-3.5" />
                <span className="truncate">{t.albums}</span>
              </button>
              <button
                type="button"
                onClick={() => openLibrarySection("artists")}
                className={navItemClass(
                  snapshot.active_nav.kind === "Library" && librarySection === "artists",
                )}
              >
                <Mic2 className="h-3.5 w-3.5" />
                <span className="truncate">{t.artists}</span>
              </button>
              <button
                type="button"
                onClick={() => openLibrarySection("downloads")}
                className={navItemClass(
                  snapshot.active_nav.kind === "Library" && librarySection === "downloads",
                )}
              >
                <Download className="h-3.5 w-3.5" />
                <span className="truncate">{t.downloads}</span>
              </button>
              <button
                type="button"
                onClick={() => openLibrarySection("local_servers")}
                className={navItemClass(
                  snapshot.active_nav.kind === "Library" && librarySection === "local_servers",
                )}
              >
                <FolderOpen className="h-3.5 w-3.5" />
                <span className="truncate">{t.localAndServers}</span>
              </button>
            </div>
          )}
        </div>

        {/* Playlists section */}
        <div>
          <button
            type="button"
            onClick={() => setPlaylistsExpanded((v) => !v)}
            className="flex w-full items-center justify-between px-2.5 py-1 text-[11px] font-semibold tracking-wide text-[var(--ink-muted)] hover:text-[var(--ink-secondary)]"
          >
            <span>{t.playlists}</span>
            {playlistsExpanded ? (
              <ChevronDown className="h-3.5 w-3.5" />
            ) : (
              <ChevronRight className="h-3.5 w-3.5" />
            )}
          </button>
          {playlistsExpanded && (
            <div className="mt-1 space-y-0.5">
              <button
                type="button"
                onClick={() =>
                  setModal({
                    type: "createPlaylist",
                    kind: snapshot.signed_in_browser && !snapshot.config.personal_only_mode
                      ? "ytm"
                      : "personal",
                  })
                }
                className={navItemClass(false)}
              >
                <Plus className="h-3.5 w-3.5" />
                <span className="truncate">{t.createNew}</span>
              </button>
              <button
                type="button"
                onClick={() => openLibrarySection("all_playlists")}
                className={navItemClass(
                  snapshot.active_nav.kind === "Library" && librarySection === "all_playlists",
                )}
              >
                <ListMusic className="h-3.5 w-3.5" />
                <span className="truncate">{t.allPlaylists}</span>
              </button>
              {snapshot.ytm_playlists
                .filter((p) => p.id === "VLLM" || p.id === "LM")
                .map((pl) => (
                  <button
                    key={pl.id}
                    type="button"
                    onClick={() =>
                      void dispatch({
                        type: "OpenCollection",
                        id: pl.id,
                        title: pl.name,
                        subtitle: "YouTube Music",
                        thumbnail_url: pl.thumbnail_url,
                        editable: true,
                      })
                    }
                    className={navItemClass(
                      snapshot.active_nav.kind === "Playlist" && snapshot.playlist_id === pl.id,
                    )}
                  >
                    <Heart className="h-3.5 w-3.5 text-[var(--accent)]" />
                    <span className="truncate">{pl.name}</span>
                  </button>
                ))}

              {snapshot.personal_playlists.map((pl) => (
                <button
                  key={pl.id}
                  type="button"
                  onClick={() =>
                    void dispatch({
                      type: "OpenCollection",
                      id: pl.id,
                      title: pl.name,
                      subtitle: `${pl.tracks.length} ${t.tracks} • ${t.localTag}`,
                      editable: true,
                    })
                  }
                  className={navItemClass(
                    snapshot.active_nav.kind === "Playlist" && snapshot.playlist_id === pl.id,
                  )}
                >
                  <ListMusic className="h-3.5 w-3.5 shrink-0 text-[var(--ink-muted)]" />
                  <span className="truncate">{pl.name}</span>
                </button>
              ))}

              {snapshot.ytm_playlists
                .filter((p) => p.id !== "VLLM" && p.id !== "LM")
                .map((pl) => (
                  <button
                    key={pl.id}
                    type="button"
                    onClick={() =>
                      void dispatch({
                        type: "OpenCollection",
                        id: pl.id,
                        title: pl.name,
                        subtitle: "YouTube Music",
                        thumbnail_url: pl.thumbnail_url,
                        editable: pl.is_owned,
                      })
                    }
                    className={navItemClass(
                      snapshot.active_nav.kind === "Playlist" && snapshot.playlist_id === pl.id,
                    )}
                  >
                    <ListMusic className="h-3.5 w-3.5 shrink-0 text-[var(--ink-muted)]" />
                    <span className="truncate">{pl.name}</span>
                  </button>
                ))}
            </div>
          )}
        </div>
      </div>

      {/* Bottom pinned profile & settings pill */}
      <div className="p-2.5 pt-1">
        <div className="flex items-center gap-1.5">
          <button
            type="button"
            onClick={() =>
              snapshot.signed_in_browser
                ? void navigate({ kind: "Settings" })
                : setModal({ type: "login" })
            }
            className="flex flex-1 min-w-0 items-center gap-2 rounded-xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-2.5 py-1.5 text-left hover:bg-[var(--bg-hover)] transition-colors"
          >
            {snapshot.user_avatar_path || snapshot.user_profile?.avatar_url ? (
              <ArtworkImage
                src={snapshot.user_avatar_path || snapshot.user_profile?.avatar_url}
                alt={t.accountAvatar}
                className="h-6 w-6"
                rounded="rounded-full"
              />
            ) : (
              <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-[var(--bg-hover)] text-[var(--ink-secondary)]">
                <User className="h-3.5 w-3.5" />
              </div>
            )}
            <span className="truncate text-xs font-medium text-[var(--ink-primary)]">
              {snapshot.user_profile?.name ||
                (snapshot.signed_in_browser ? snapshot.signed_in_browser : t.signIn)}
            </span>
          </button>

          <button
            type="button"
            onClick={() => void navigate({ kind: "Settings" })}
            aria-label={t.settings}
            className={cn(
              "flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] text-[var(--ink-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--ink-primary)] transition-colors",
              snapshot.active_nav.kind === "Settings" && "text-[var(--ink-primary)] bg-[var(--bg-hover)]",
            )}
          >
            <Settings className="h-4 w-4" />
          </button>
        </div>
      </div>
    </aside>
  );
}

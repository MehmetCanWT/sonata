import {
  ChevronLeft,
  ChevronRight,
  ListMusic,
  MessageSquareQuote,
  MoreHorizontal,
  PanelLeft,
  Plus,
  RefreshCw,
  Settings,
} from "lucide-react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { WindowControls } from "@/components/WindowControls";
import { getStrings } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { useSonataStore } from "@/store/useSonataStore";

interface TopBarProps {
  heroOverlay?: boolean;
}

export function TopBar({ heroOverlay = false }: TopBarProps) {
  const language = useSonataStore((s) => s.snapshot.config.language);
  const queueLen = useSonataStore((s) => s.snapshot.queue.length);
  const currentTrack = useSonataStore((s) =>
    s.snapshot.current_index != null ? s.snapshot.queue[s.snapshot.current_index] : undefined,
  );
  const showWindowControls = useSonataStore((s) => s.showWindowControls);
  const queueOpen = useSonataStore((s) => s.queueOpen);
  const sidebarOpen = useSonataStore((s) => s.sidebarOpen);
  const immersiveOpen = useSonataStore((s) => s.immersiveOpen);
  const canGoBack = useSonataStore((s) => s.navBack.length > 0);
  const canGoForward = useSonataStore((s) => s.navForward.length > 0);
  const goBack = useSonataStore((s) => s.goBack);
  const goForward = useSonataStore((s) => s.goForward);
  const toggleSidebar = useSonataStore((s) => s.toggleSidebar);
  const toggleQueue = useSonataStore((s) => s.toggleQueue);
  const setImmersiveOpen = useSonataStore((s) => s.setImmersiveOpen);
  const setModal = useSonataStore((s) => s.setModal);
  const navigate = useSonataStore((s) => s.navigate);
  const dispatch = useSonataStore((s) => s.dispatch);

  const t = getStrings(language);
  const hasQueue = queueLen > 0;

  const pillBtn = cn(
    "flex size-8 items-center justify-center rounded-full border transition-colors",
    "disabled:opacity-30 disabled:pointer-events-none",
    heroOverlay
      ? "border-white/15 bg-black/45 text-white/85 hover:bg-black/65 hover:text-white"
      : "border-[var(--pill-border)] bg-[var(--pill-bg)] text-[var(--ink-secondary)] hover:bg-[var(--pill-bg-hover)] hover:text-[var(--ink-primary)]",
  );

  return (
    <header
      className={cn(
        "z-30 flex items-center justify-between gap-3 px-4 select-none",
        heroOverlay
          ? "pointer-events-none absolute inset-x-0 top-0 py-3 bg-gradient-to-b from-black/45 via-black/18 to-transparent"
          : "relative h-14 shrink-0 border-b border-[var(--border-hairline)] bg-[var(--bg-surface)]",
      )}
      data-tauri-drag-region
    >
      {/* Left navigation & sidebar toggle */}
      <div className="pointer-events-auto flex items-center gap-1.5" data-no-drag>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button type="button" aria-label={t.menu} className={pillBtn}>
              <MoreHorizontal className="size-4" />
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start">
            <DropdownMenuItem onSelect={() => void dispatch({ type: "RefreshHome" })}>
              <RefreshCw className="size-3.5" />
              {t.refreshHome}
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => void dispatch({ type: "StartQuickRadio" })}>
              {t.startQuickRadio}
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem onSelect={() => setModal({ type: "sleepTimer" })}>
              {t.sleepTimer}
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => void navigate({ kind: "Settings" })}>
              <Settings className="size-3.5" />
              {t.settings}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

        <div
          className={cn(
            "flex items-center rounded-full border p-0.5",
            heroOverlay
              ? "border-white/15 bg-black/45"
              : "border-[var(--pill-border)] bg-[var(--pill-bg)]",
          )}
        >
          <button
            type="button"
            onClick={() => void goBack()}
            disabled={!canGoBack}
            aria-label={t.back}
            title={`${t.back} (Alt+←)`}
            className={cn(
              "flex size-7 items-center justify-center rounded-full transition-colors disabled:opacity-30 disabled:pointer-events-none",
              heroOverlay
                ? "text-white/85 hover:bg-white/10 hover:text-white"
                : "text-[var(--ink-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--ink-primary)]",
            )}
          >
            <ChevronLeft className="size-4" />
          </button>
          <button
            type="button"
            onClick={() => void goForward()}
            disabled={!canGoForward}
            aria-label={t.forward}
            title={`${t.forward} (Alt+→)`}
            className={cn(
              "flex size-7 items-center justify-center rounded-full transition-colors disabled:opacity-30 disabled:pointer-events-none",
              heroOverlay
                ? "text-white/85 hover:bg-white/10 hover:text-white"
                : "text-[var(--ink-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--ink-primary)]",
            )}
          >
            <ChevronRight className="size-4" />
          </button>
        </div>

        <button
          type="button"
          onClick={toggleSidebar}
          aria-label={t.toggleSidebar}
          className={cn(
            pillBtn,
            sidebarOpen &&
              (heroOverlay
                ? "text-white border-white/25 bg-black/60"
                : "text-[var(--ink-primary)] border-[var(--border-strong)]"),
          )}
        >
          <PanelLeft className="size-4" />
        </button>
      </div>

      {/* Right actions */}
      <div className="pointer-events-auto flex items-center gap-1.5" data-no-drag>
        <button
          type="button"
          onClick={() =>
            currentTrack
              ? setModal({ type: "addToPlaylist", track: currentTrack })
              : setModal({ type: "createPlaylist", kind: "personal" })
          }
          aria-label={t.quickAdd}
          className={pillBtn}
        >
          <Plus className="size-4" />
        </button>

        <button
          type="button"
          onClick={() => setImmersiveOpen(!immersiveOpen)}
          aria-label={t.lyrics}
          className={cn(
            pillBtn,
            immersiveOpen &&
              (heroOverlay
                ? "text-white border-white/30 bg-black/65"
                : "text-[var(--accent)] border-[var(--border-strong)]"),
          )}
        >
          <MessageSquareQuote className="size-4" />
        </button>

        <button
          type="button"
          onClick={toggleQueue}
          aria-label={t.toggleQueue}
          className={cn(
            pillBtn,
            "relative",
            queueOpen &&
              (heroOverlay
                ? "text-white border-white/30 bg-black/65"
                : "text-[var(--accent)] border-[var(--border-strong)]"),
          )}
        >
          <ListMusic className="size-4" />
          {hasQueue && (
            <span className="absolute right-1.5 top-1.5 size-1.5 rounded-full bg-[var(--accent)]" />
          )}
        </button>

        {showWindowControls && !queueOpen && (
          <>
            <div className="mx-1 h-4 w-px bg-[var(--border-hairline)]" />
            <WindowControls />
          </>
        )}
      </div>
    </header>
  );
}

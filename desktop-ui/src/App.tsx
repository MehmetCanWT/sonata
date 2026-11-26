import { useEffect, useRef, useState } from "react";
import { Modals } from "@/components/Modals";
import { PlayerDock } from "@/components/PlayerDock";
import { QueuePanel } from "@/components/QueuePanel";
import { Sidebar } from "@/components/Sidebar";
import { TopBar } from "@/components/TopBar";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { TooltipProvider } from "@/components/ui/tooltip";
import { WindowControls } from "@/components/WindowControls";
import { getStrings } from "@/lib/i18n";
import { useSonataStore } from "@/store/useSonataStore";
import { AlbumView } from "@/views/AlbumView";
import { ArtistView } from "@/views/ArtistView";
import { ExploreView } from "@/views/ExploreView";
import { HistoryView } from "@/views/HistoryView";
import { HomeView } from "@/views/HomeView";
import { ImmersiveLyricsView } from "@/views/ImmersiveLyricsView";
import { LibraryView } from "@/views/LibraryView";
import { RadioView } from "@/views/RadioView";
import { SearchView } from "@/views/SearchView";
import { SettingsView } from "@/views/SettingsView";
import { StatsView } from "@/views/StatsView";

function useWindowWidth(): number {
  const [width, setWidth] = useState(() =>
    typeof window !== "undefined" ? window.innerWidth : 1280,
  );
  useEffect(() => {
    const onResize = () => setWidth(window.innerWidth);
    window.addEventListener("resize", onResize, { passive: true });
    return () => window.removeEventListener("resize", onResize);
  }, []);
  return width;
}

function CenterContent() {
  const nav = useSonataStore((s) => s.snapshot.active_nav);
  switch (nav.kind) {
    case "Home":
      return <HomeView />;
    case "Explore":
      return <ExploreView mode="explore" />;
    case "MoodCategory":
      return <ExploreView mode="moodCategory" />;
    case "ShelfDetail":
      return <ExploreView mode="shelfDetail" />;
    case "Radio":
      return <RadioView />;
    case "Library":
      return <LibraryView />;
    case "History":
      return <HistoryView />;
    case "Stats":
      return <StatsView />;
    case "Search":
      return <SearchView />;
    case "Album":
      return <AlbumView mode="album" />;
    case "Playlist":
      return <AlbumView mode="playlist" />;
    case "Artist":
      return <ArtistView />;
    case "Settings":
      return <SettingsView />;
    case "Queue":
      return <HomeView />;
  }
}

export function App() {
  const init = useSonataStore((s) => s.init);
  const sidebarOpen = useSonataStore((s) => s.sidebarOpen);
  const setSidebarOpen = useSonataStore((s) => s.setSidebarOpen);
  const queueOpen = useSonataStore((s) => s.queueOpen);
  const toggleQueue = useSonataStore((s) => s.toggleQueue);
  const immersiveOpen = useSonataStore((s) => s.immersiveOpen);
  const showWindowControls = useSonataStore((s) => s.showWindowControls);
  const activeNavKind = useSonataStore((s) => s.snapshot.active_nav.kind);
  const language = useSonataStore((s) => s.snapshot.config.language);
  const pendingPair = useSonataStore((s) => s.snapshot.connect_pending_pair);
  const dispatch = useSonataStore((s) => s.dispatch);
  const t = getStrings(language);
  const navigate = useSonataStore((s) => s.navigate);
  const goBack = useSonataStore((s) => s.goBack);
  const goForward = useSonataStore((s) => s.goForward);
  const setImmersiveOpen = useSonataStore((s) => s.setImmersiveOpen);

  const windowWidth = useWindowWidth();
  const isCompactSidebar = windowWidth < 860;
  const isCompactQueue = windowWidth < 1180;
  const prevWidthRef = useRef(windowWidth);

  useEffect(() => {
    const prev = prevWidthRef.current;
    prevWidthRef.current = windowWidth;
    if (prev >= 860 && windowWidth < 860) {
      setSidebarOpen(false);
    } else if (prev < 860 && windowWidth >= 860) {
      setSidebarOpen(true);
    }
  }, [windowWidth, setSidebarOpen]);

  useEffect(() => {
    let cleanup: (() => void) | undefined;
    let active = true;
    void init().then((fn) => {
      if (active) {
        cleanup = fn;
      } else {
        fn();
      }
    });
    return () => {
      active = false;
      cleanup?.();
    };
  }, [init]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const inInput =
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable);

      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        void navigate({ kind: "Search" });
        return;
      }
      if (e.altKey && e.key === "ArrowLeft") {
        e.preventDefault();
        void goBack();
        return;
      }
      if (e.altKey && e.key === "ArrowRight") {
        e.preventDefault();
        void goForward();
        return;
      }
      if (inInput) return;

      if (e.code === "Space") {
        e.preventDefault();
        void dispatch({ type: "TogglePause" });
      } else if (e.key === "ArrowRight" && e.shiftKey) {
        e.preventDefault();
        void dispatch({ type: "NextTrack" });
      } else if (e.key === "ArrowLeft" && e.shiftKey) {
        e.preventDefault();
        void dispatch({ type: "PrevTrack" });
      } else if (e.key === "ArrowRight") {
        void dispatch({ type: "SeekRelative", secs: 5 });
      } else if (e.key === "ArrowLeft") {
        void dispatch({ type: "SeekRelative", secs: -5 });
      } else if (e.key.toLowerCase() === "m") {
        void dispatch({ type: "ToggleMute" });
      } else if (e.key.toLowerCase() === "q") {
        toggleQueue();
      } else if (e.key.toLowerCase() === "l") {
        setImmersiveOpen(!useSonataStore.getState().immersiveOpen);
      }
    };
    const onMouseUp = (e: MouseEvent) => {
      if (e.button === 3) {
        e.preventDefault();
        void goBack();
      } else if (e.button === 4) {
        e.preventDefault();
        void goForward();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [dispatch, goBack, goForward, navigate, setImmersiveOpen, toggleQueue]);

  const isHeroView =
    activeNavKind === "Artist" || activeNavKind === "Album" || activeNavKind === "Playlist";

  return (
    <TooltipProvider delayDuration={350}>
      <div className="relative flex h-screen w-screen overflow-hidden bg-[var(--bg-app)] p-2 gap-2">
        {/* Top-right native window controls above the inline QueuePanel */}
        {showWindowControls && queueOpen && !isCompactQueue && (
          <div className="absolute right-3 top-2.5 z-30">
            <WindowControls />
          </div>
        )}

        {sidebarOpen && !isCompactSidebar && <Sidebar />}
        {sidebarOpen && isCompactSidebar && (
          <>
            <button
              type="button"
              aria-label={t.closeSidebar}
              onClick={() => setSidebarOpen(false)}
              className="fixed inset-0 z-25 bg-black/50"
            />
            <div className="fixed inset-y-2 left-2 z-30 h-[calc(100vh-1rem)] overflow-hidden rounded-2xl border border-[var(--border-strong)] bg-[var(--bg-sidebar)] shadow-2xl">
              <Sidebar />
            </div>
          </>
        )}

        {/* Center matte rounded-2xl main viewport */}
        <main className="isolate @container relative flex flex-1 min-w-0 flex-col overflow-hidden rounded-2xl border border-[var(--border-hairline)] bg-[var(--bg-surface)]">
          <TopBar heroOverlay={isHeroView} />
          <div className="flex-1 min-h-0">
            <CenterContent />
          </div>
          <div className="pointer-events-none absolute inset-x-0 bottom-0 z-10 h-28 bg-gradient-to-t from-[var(--bg-surface)] via-[var(--bg-surface)]/75 to-transparent" />
          <div className="pointer-events-none absolute inset-x-0 bottom-0 z-20 flex justify-center">
            <div className="pointer-events-auto w-full">
              <PlayerDock />
            </div>
          </div>
        </main>

        {queueOpen && !isCompactQueue && (
          <div className={showWindowControls ? "h-full pt-8" : "h-full"}>
            <QueuePanel />
          </div>
        )}
        {queueOpen && isCompactQueue && (
          <>
            <button
              type="button"
              aria-label={t.closeQueue}
              onClick={toggleQueue}
              className="fixed inset-0 z-25 bg-black/50"
            />
            <div className="fixed inset-y-2 right-2 z-30 h-[calc(100vh-1rem)] overflow-hidden rounded-2xl border border-[var(--border-strong)] bg-[var(--bg-sidebar)] shadow-2xl">
              <QueuePanel />
            </div>
          </>
        )}

        {immersiveOpen && <ImmersiveLyricsView />}
        <Modals />
        {pendingPair && (
          <Dialog
            open={true}
            onOpenChange={(open) => {
              if (!open) {
                void dispatch({ type: "RejectConnectPair", request_id: pendingPair.request_id });
              }
            }}
          >
            <DialogContent>
              <DialogHeader>
                <DialogTitle>{t.connectPairRequestTitle}</DialogTitle>
                <DialogDescription>
                  <span className="font-semibold text-[var(--ink-primary)]">
                    {pendingPair.device_name}
                  </span>{" "}
                  {t.connectPairRequestDesc}
                </DialogDescription>
              </DialogHeader>
              <div className="mb-5 rounded-xl border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-3.5 py-2.5 text-xs text-[var(--ink-secondary)]">
                <div className="font-medium text-[var(--ink-primary)]">
                  {pendingPair.device_name}
                </div>
                <div className="mt-0.5 font-mono text-[11px] text-[var(--ink-muted)]">
                  {pendingPair.device_id}
                </div>
              </div>
              <div className="flex justify-end gap-2">
                <Button
                  type="button"
                  variant="secondary"
                  onClick={() =>
                    void dispatch({ type: "RejectConnectPair", request_id: pendingPair.request_id })
                  }
                >
                  {t.reject}
                </Button>
                <Button
                  type="button"
                  variant="accent"
                  onClick={() =>
                    void dispatch({
                      type: "ApproveConnectPair",
                      request_id: pendingPair.request_id,
                    })
                  }
                >
                  {t.approve}
                </Button>
              </div>
            </DialogContent>
          </Dialog>
        )}
      </div>
    </TooltipProvider>
  );
}

export default App;

import { ChevronLeft, ChevronRight } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { getStrings } from "@/i18n";
import { cn } from "@/lib/utils";
import { useSonataStore } from "@/store/useSonataStore";

interface HorizontalScrollerProps {
  children: React.ReactNode;
  className?: string;
  contentClassName?: string;
  scrollStep?: number;
}

export function HorizontalScroller({
  children,
  className,
  contentClassName,
  scrollStep = 420,
}: HorizontalScrollerProps) {
  const language = useSonataStore((s) => s.snapshot.config.language);
  const t = getStrings(language);
  const scrollerRef = useRef<HTMLDivElement | null>(null);
  const [canLeft, setCanLeft] = useState(false);
  const [canRight, setCanRight] = useState(false);

  const syncOverflow = useCallback(() => {
    const el = scrollerRef.current;
    if (!el) return;
    setCanLeft(el.scrollLeft > 4);
    setCanRight(el.scrollLeft + el.clientWidth < el.scrollWidth - 4);
  }, []);

  useEffect(() => {
    syncOverflow();
    const el = scrollerRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(syncOverflow);
    ro.observe(el);
    return () => ro.disconnect();
  }, [syncOverflow, children]);

  const scrollByDir = (dir: -1 | 1) => {
    const el = scrollerRef.current;
    if (!el) return;
    el.scrollBy({ left: dir * scrollStep, behavior: "smooth" });
  };

  return (
    <div className={cn("group/scroller relative shrink-0", className)}>
      <div
        ref={scrollerRef}
        onScroll={syncOverflow}
        onWheel={(e) => {
          const el = scrollerRef.current;
          if (!el) return;
          if (Math.abs(e.deltaX) > Math.abs(e.deltaY)) return;
          if (e.shiftKey && el.scrollWidth > el.clientWidth) {
            e.preventDefault();
            el.scrollLeft += e.deltaY;
          }
        }}
        className={cn(
          "no-scrollbar flex items-center overflow-x-auto scroll-smooth",
          contentClassName,
        )}
      >
        {children}
      </div>

      {canLeft && (
        <button
          type="button"
          onClick={() => scrollByDir(-1)}
          aria-label={t.scrollLeft}
          className="absolute left-1 top-1/2 z-10 -translate-y-1/2 flex size-8 items-center justify-center rounded-full border border-[var(--pill-border)] bg-[var(--pill-bg)] text-[var(--ink-primary)] opacity-0 shadow-md transition-opacity group-hover/scroller:opacity-100 hover:bg-[var(--pill-bg-hover)]"
        >
          <ChevronLeft className="size-4" />
        </button>
      )}
      {canRight && (
        <button
          type="button"
          onClick={() => scrollByDir(1)}
          aria-label={t.scrollRight}
          className="absolute right-1 top-1/2 z-10 -translate-y-1/2 flex size-8 items-center justify-center rounded-full border border-[var(--pill-border)] bg-[var(--pill-bg)] text-[var(--ink-primary)] opacity-0 shadow-md transition-opacity group-hover/scroller:opacity-100 hover:bg-[var(--pill-bg-hover)]"
        >
          <ChevronRight className="size-4" />
        </button>
      )}
    </div>
  );
}

import { Minus, Square, X } from "lucide-react";
import { getStrings } from "@/i18n";
import { windowClose, windowMinimize, windowToggleMaximize } from "@/lib/ipc";
import { useSonataStore } from "@/store/useSonataStore";

export function WindowControls() {
  const showWindowControls = useSonataStore((s) => s.showWindowControls);
  const language = useSonataStore((s) => s.snapshot.config.language);
  const t = getStrings(language);

  if (!showWindowControls) {
    return null;
  }

  return (
    <div className="flex items-center gap-1 select-none" data-no-drag>
      <button
        type="button"
        onClick={() => void windowMinimize()}
        aria-label={t.minimizeWindow}
        className="flex h-7 w-7 items-center justify-center rounded-md text-[var(--ink-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--ink-primary)] transition-colors"
      >
        <Minus className="h-3.5 w-3.5" />
      </button>
      <button
        type="button"
        onClick={() => void windowToggleMaximize()}
        aria-label={t.maximizeWindow}
        className="flex h-7 w-7 items-center justify-center rounded-md text-[var(--ink-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--ink-primary)] transition-colors"
      >
        <Square className="h-3 w-3" />
      </button>
      <button
        type="button"
        onClick={() => void windowClose()}
        aria-label={t.closeWindow}
        className="flex h-7 w-7 items-center justify-center rounded-md text-[var(--ink-secondary)] hover:bg-red-500/85 hover:text-white transition-colors"
      >
        <X className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}

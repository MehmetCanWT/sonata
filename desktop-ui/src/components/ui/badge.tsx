import * as React from "react";
import { cn } from "@/lib/utils";

export function Badge({
  className,
  ...props
}: React.HTMLAttributes<HTMLSpanElement>) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-full border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-2.5 py-0.5 text-[11px] font-medium uppercase tracking-wider text-[var(--ink-secondary)]",
        className,
      )}
      {...props}
    />
  );
}

import * as React from "react";
import { cn } from "@/lib/utils";

export const Input = React.forwardRef<
  HTMLInputElement,
  React.InputHTMLAttributes<HTMLInputElement>
>(({ className, type, ...props }, ref) => (
  <input
    type={type}
    ref={ref}
    className={cn(
      "flex h-9 w-full rounded-lg border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-3 py-1.5 text-sm text-[var(--ink-primary)] placeholder:text-[var(--ink-muted)] transition-colors focus-visible:outline-none focus-visible:border-[var(--border-strong)] disabled:cursor-not-allowed disabled:opacity-50",
      className,
    )}
    {...props}
  />
));
Input.displayName = "Input";

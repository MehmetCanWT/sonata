import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "matte-btn inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-lg text-sm font-medium disabled:pointer-events-none disabled:opacity-45 [&_svg]:pointer-events-none [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default:
          "bg-[var(--ink-primary)] text-[var(--bg-surface)] hover:opacity-90",
        accent:
          "bg-[var(--accent)] text-[var(--accent-ink)] hover:brightness-110",
        secondary:
          "bg-[var(--bg-elevated)] text-[var(--ink-primary)] border border-[var(--border-hairline)] hover:bg-[var(--bg-hover)]",
        ghost:
          "text-[var(--ink-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--ink-primary)]",
        outline:
          "border border-[var(--border-hairline)] bg-transparent text-[var(--ink-primary)] hover:bg-[var(--bg-hover)]",
        destructive:
          "bg-red-600/90 text-white hover:bg-red-600",
      },
      size: {
        default: "h-9 px-4 py-2",
        sm: "h-8 px-3 text-xs",
        lg: "h-10 px-6",
        pill: "h-9 px-5 rounded-full",
        icon: "h-8 w-8 rounded-lg",
        "icon-sm": "h-7 w-7 rounded-md",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {}

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, ...props }, ref) => (
    <button
      ref={ref}
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  ),
);
Button.displayName = "Button";

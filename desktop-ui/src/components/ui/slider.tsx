import * as React from "react";
import * as SliderPrimitive from "@radix-ui/react-slider";
import { cn } from "@/lib/utils";

interface SliderProps
  extends React.ComponentPropsWithoutRef<typeof SliderPrimitive.Root> {
  trackClassName?: string;
  rangeClassName?: string;
  showThumb?: boolean;
}

export const Slider = React.forwardRef<
  React.ElementRef<typeof SliderPrimitive.Root>,
  SliderProps
>(({ className, trackClassName, rangeClassName, showThumb = true, ...props }, ref) => (
  <SliderPrimitive.Root
    ref={ref}
    className={cn(
      "group relative flex w-full touch-none select-none items-center cursor-pointer",
      className,
    )}
    {...props}
  >
    <SliderPrimitive.Track
      className={cn(
        "relative h-1 w-full grow overflow-hidden rounded-full bg-white/15 group-hover:h-1.5 transition-[height]",
        trackClassName,
      )}
    >
      <SliderPrimitive.Range
        className={cn(
          "absolute h-full bg-[var(--ink-primary)] transition-colors",
          rangeClassName,
        )}
      />
    </SliderPrimitive.Track>
    {showThumb && (
      <SliderPrimitive.Thumb className="block h-3 w-3 rounded-full bg-[var(--ink-primary)] opacity-0 shadow-sm transition-opacity group-hover:opacity-100 focus-visible:opacity-100 focus-visible:outline-none" />
    )}
  </SliderPrimitive.Root>
));
Slider.displayName = SliderPrimitive.Root.displayName;

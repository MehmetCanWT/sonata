import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatDuration(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds) || seconds < 0) {
    return "0:00";
  }
  const total = Math.floor(seconds);
  const hrs = Math.floor(total / 3600);
  const mins = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  if (hrs > 0) {
    return `${hrs}:${String(mins).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
  }
  return `${mins}:${String(secs).padStart(2, "0")}`;
}

export function formatRemainingDuration(remainingSecs: number): string {
  if (!Number.isFinite(remainingSecs) || remainingSecs <= 0) {
    return "-0:00";
  }
  return `-${formatDuration(remainingSecs)}`;
}

export function formatQueueEta(totalRemainingSecs: number): { durationLabel: string; finishTimeLabel: string } {
  const safeSecs = Math.max(0, Math.round(totalRemainingSecs));
  const minsTotal = Math.round(safeSecs / 60);
  const hrs = Math.floor(minsTotal / 60);
  const mins = minsTotal % 60;
  const durationLabel = hrs > 0 ? `${hrs}h ${mins}m` : `${mins}m`;
  const etaDate = new Date(Date.now() + safeSecs * 1000);
  const finishTimeLabel = etaDate.toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });
  return { durationLabel, finishTimeLabel };
}

/**
 * Normalizes extracted RGB tuples into safe matte accents, rejecting violet/lavender
 * hue ranges (265..335 deg) per design rules and clamping excessive neon saturation.
 */
export function sanitizeAccentRgb(rgb: [number, number, number] | null | undefined): [number, number, number] {
  if (!rgb) return [214, 152, 118];
  let [r, g, b] = rgb.map((v) => Math.max(0, Math.min(255, v))) as [number, number, number];
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const d = max - min;
  if (d < 0.04) {
    return [214, 152, 118];
  }
  let hue = 0;
  if (max === rn) hue = ((gn - bn) / d + (gn < bn ? 6 : 0)) * 60;
  else if (max === gn) hue = ((bn - rn) / d + 2) * 60;
  else hue = ((rn - gn) / d + 4) * 60;

  // Guard against violet / lavender hues: shift cleanly toward warm terracotta or cool slate blue.
  if (hue >= 260 && hue <= 335) {
    return hue < 295 ? [96, 148, 224] : [224, 110, 128];
  }

  // Lift dark artwork colors slightly so they remain visible on #111114 surfaces.
  const lum = 0.2126 * rn + 0.7152 * gn + 0.0722 * bn;
  if (lum < 0.28) {
    const boost = 0.28 / Math.max(lum, 0.06);
    const factor = Math.min(boost, 2.2);
    r = Math.min(240, Math.round(r * factor + 28));
    g = Math.min(240, Math.round(g * factor + 22));
    b = Math.min(240, Math.round(b * factor + 22));
  }
  return [r, g, b];
}

export function sanitizeAmbientBgRgb(rgb: [number, number, number] | null | undefined): [number, number, number] {
  const [r, g, b] = sanitizeAccentRgb(rgb);
  return [
    Math.max(36, Math.round(r * 0.65)),
    Math.max(18, Math.round(g * 0.55)),
    Math.max(18, Math.round(b * 0.55)),
  ];
}

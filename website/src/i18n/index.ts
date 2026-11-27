import { en } from "./en";
import { ru } from "./ru";
import { tr } from "./tr";
import type { SiteLocale, SiteStrings } from "./types";

export const SITE_STRINGS: Record<SiteLocale, SiteStrings> = {
  en,
  tr,
  ru,
};

export const LOCALES: SiteLocale[] = ["en", "tr", "ru"];

export function nextLocale(current: SiteLocale): SiteLocale {
  const idx = LOCALES.indexOf(current);
  return LOCALES[(idx + 1) % LOCALES.length];
}

export function getInitialLocale(): SiteLocale {
  try {
    const saved = localStorage.getItem("sonata-site-lang");
    if (saved === "en" || saved === "tr" || saved === "ru") {
      return saved;
    }
  } catch {
    // Ignore storage access errors.
  }
  return "en";
}

export type { SiteLocale, SiteStrings, PlatformId } from "./types";

import { EN } from "@/i18n/en";
import { RU } from "@/i18n/ru";
import { TR } from "@/i18n/tr";
import type { UiStrings } from "@/i18n/types";
import type { AppLanguage } from "@/types/sonata";

export type { UiStrings } from "@/i18n/types";

export const SUPPORTED_LANGUAGES: ReadonlyArray<{ code: AppLanguage; label: string }> = [
  { code: "en", label: "English" },
  { code: "tr", label: "Türkçe" },
  { code: "ru", label: "Русский" },
];

export function getStrings(lang: AppLanguage): UiStrings {
  switch (lang) {
    case "tr":
      return TR;
    case "ru":
      return RU;
    default:
      return EN;
  }
}

export function getLanguageLabel(lang: AppLanguage): string {
  return SUPPORTED_LANGUAGES.find((entry) => entry.code === lang)?.label ?? "English";
}

// web/src/stores/locale-store.ts
import { create } from "zustand";

export type AppLocale = "fr" | "en";

const STORAGE_KEY = "talaria.locale";

function detectLocale(): AppLocale {
  if (typeof window === "undefined") return "en";
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    if (saved === "fr" || saved === "en") return saved;
  } catch {
    /* ignore */
  }
  const language = window.navigator.language?.toLowerCase() ?? "en";
  return language.startsWith("fr") ? "fr" : "en";
}

interface LocaleState {
  locale: AppLocale;
  setLocale: (locale: AppLocale) => void;
}

export const useLocaleStore = create<LocaleState>((set) => ({
  locale: detectLocale(),
  setLocale: (locale) => {
    try {
      window.localStorage.setItem(STORAGE_KEY, locale);
    } catch {
      /* ignore */
    }
    set({ locale });
  },
}));

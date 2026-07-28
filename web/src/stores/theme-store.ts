// web/src/stores/theme-store.ts
import { create } from "zustand";

interface ThemeState {
  theme: "dark" | "light";
}

export const useThemeStore = create<ThemeState>(() => ({
  theme: "dark",
}));

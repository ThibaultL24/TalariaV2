import { create } from "zustand";
import { persist } from "zustand/middleware";
type Theme = "light" | "dark";
type Preference = Theme | "system";
interface ThemeState {
  theme: Theme;
  preference: Preference;
  setTheme: (preference: Preference) => void;
  syncSystem: () => void;
}
function resolve(preference: Preference): Theme {
  return preference === "system"
    ? window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light"
    : preference;
}
export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      theme: "dark",
      preference: "dark",
      setTheme: (preference) => set({ preference, theme: resolve(preference) }),
      syncSystem: () => set({ theme: resolve(get().preference) }),
    }),
    {
      name: "talaria-theme-v3",
      partialize: (state) => ({ preference: state.preference }),
    },
  ),
);
export function initializeTheme(): () => void {
  const sync = () => {
    document.documentElement.dataset.theme = useThemeStore.getState().theme;
  };
  useThemeStore.getState().syncSystem();
  sync();
  const unsubscribe = useThemeStore.subscribe(sync);
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const onChange = () => useThemeStore.getState().syncSystem();
  media.addEventListener("change", onChange);
  return () => {
    unsubscribe();
    media.removeEventListener("change", onChange);
  };
}

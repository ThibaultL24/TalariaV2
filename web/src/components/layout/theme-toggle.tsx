// web/src/components/layout/theme-toggle.tsx
import { useThemeStore } from "@/stores/theme-store";

export function ThemeToggle() {
  const theme = useThemeStore((s) => s.theme);
  const preference = useThemeStore((s) => s.preference);
  const setTheme = useThemeStore((s) => s.setTheme);

  const label =
    preference === "system"
      ? `Theme: system (${theme})`
      : theme === "dark"
        ? "Switch to light mode"
        : "Switch to dark mode";

  function cycle() {
    if (preference === "system") {
      setTheme(theme === "dark" ? "light" : "dark");
      return;
    }
    setTheme(theme === "dark" ? "light" : "dark");
  }

  return (
    <button
      type="button"
      className="theme-toggle"
      onClick={cycle}
      aria-label={label}
      title={label}
    >
      {theme === "dark" ? (
        <span aria-hidden="true">☀</span>
      ) : (
        <span aria-hidden="true">☾</span>
      )}
    </button>
  );
}

// web/src/components/layout/theme-toggle.test.tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { ThemeToggle } from "./theme-toggle";
import { useThemeStore } from "@/stores/theme-store";

describe("ThemeToggle", () => {
  beforeEach(() => {
    useThemeStore.setState({ theme: "light", preference: "light" });
    document.documentElement.dataset.theme = "light";
    useThemeStore.subscribe((state) => {
      document.documentElement.dataset.theme = state.theme;
    });
  });

  it("toggles light/dark via Zustand and data-theme", () => {
    render(<ThemeToggle />);
    fireEvent.click(screen.getByRole("button", { name: /switch to dark mode/i }));
    expect(useThemeStore.getState().theme).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    fireEvent.click(screen.getByRole("button", { name: /switch to light mode/i }));
    expect(useThemeStore.getState().theme).toBe("light");
    expect(document.documentElement.dataset.theme).toBe("light");
  });
});

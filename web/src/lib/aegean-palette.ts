// web/src/lib/aegean-palette.ts — shared Aegean Day/Night hex for Pixi & map tooling

export type AegeanTheme = "light" | "dark";

export const AEGEAN_HEX = {
  light: {
    canvas: "#F7F3EA",
    ink: "#20313A",
    primary: "#0F5F7A",
    selected: "#E76F3C",
  },
  dark: {
    canvas: "#08151C",
    ink: "#F2EEE5",
    primary: "#45A6C4",
    selected: "#F07A45",
  },
} as const;

function hexToPixi(hex: string): number {
  const n = hex.replace("#", "");
  return Number.parseInt(n, 16);
}

export function aegeanPixiColors(theme: AegeanTheme) {
  const h = AEGEAN_HEX[theme];
  return {
    background: hexToPixi(h.canvas),
    guideInk: hexToPixi(h.ink),
    primary: hexToPixi(h.primary),
    selectedRing: hexToPixi(h.selected),
  };
}

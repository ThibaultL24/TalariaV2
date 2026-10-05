// web/src/map/map-theme.test.ts
import { describe, expect, it } from "vitest";
import { MAP_LAYER_PALETTE, OPENFREEMAP_DARK_STYLE, pickMapStyle } from "./map-theme";
import { ANTIQUE_MAP_STYLE } from "@/styles/map-style-antique";

describe("map-theme", () => {
  it("selects antique style for light and OpenFreeMap for dark", () => {
    expect(pickMapStyle(false)).toBe(ANTIQUE_MAP_STYLE);
    expect(pickMapStyle(true)).toBe(OPENFREEMAP_DARK_STYLE);
  });

  it("uses Aegean sunset orange for selected events in both palettes", () => {
    expect(MAP_LAYER_PALETTE.light.selected).toBe("#E76F3C");
    expect(MAP_LAYER_PALETTE.dark.selected).toBe("#F07A45");
    expect(MAP_LAYER_PALETTE.light.cluster).toBe("#0F5F7A");
    expect(MAP_LAYER_PALETTE.dark.cluster).toBe("#45A6C4");
  });
});

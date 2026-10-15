// web/src/lib/map-preview.test.ts
import { describe, expect, it } from "vitest";
import { isClusterFeature, previewFromFeature } from "@/lib/map-preview";
import type { TalariaFeature } from "@/lib/schemas/geojson";

function feature(properties: Record<string, unknown>): TalariaFeature {
  return {
    type: "Feature",
    geometry: { type: "Point", coordinates: [2, 48] },
    properties,
  };
}

describe("map preview", () => {
  it("reads title, typed date and category from loaded properties", () => {
    const preview = previewFromFeature(
      feature({
        id: "evt-1",
        title: "Arrivée à Moscou",
        event_type: "travel",
        time: { kind: "exact", precision: "day", surface: "14 septembre 1812", start: "1812-09-14" },
        summary: "Entrée dans la ville.",
      }),
      "fr",
      "Date inconnue",
    );
    expect(preview).toMatchObject({
      id: "evt-1",
      title: "Arrivée à Moscou",
      when: "14 septembre 1812",
      category: "Voyage",
      summary: "Entrée dans la ville.",
    });
  });

  it("keeps unknown dates unknown", () => {
    const preview = previewFromFeature(
      feature({ id: "evt-2", title: "Rencontre", event_type: "meeting", time: { kind: "unknown" } }),
      "fr",
      "Date inconnue",
    );
    expect(preview?.when).toBe("Date inconnue");
  });

  it("does not invent a preview for a cluster", () => {
    const cluster = feature({ point_count: 12, title: "should not show" });
    expect(isClusterFeature(cluster)).toBe(true);
    expect(previewFromFeature(cluster, "en", "Unknown date")).toBeNull();
  });
});

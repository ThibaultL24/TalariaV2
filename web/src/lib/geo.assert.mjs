// web/src/lib/geo.assert.mjs
import assert from "node:assert/strict";
import { boundsOfMapFeatures, buildYearBounds, spreadStackedMapPoints } from "./geo.ts";

function ev(event_type, year, extra = {}) {
  return {
    event_type,
    start_time: `${year}-01-01`,
    time: { kind: "exact", start: `${year}-01-01`, precision: "year" },
    ...extra,
  };
}

assert.deepEqual(
  buildYearBounds([
    ev("birth", 1769),
    ev("battle", 1815),
    ev("death", 1821),
    ev("memorial", 1840),
  ]),
  { min: 1769, max: 1821 },
);

assert.deepEqual(
  buildYearBounds([ev("birth", 1950), ev("office", 2001)]),
  { min: 1950, max: 2001 },
);

assert.deepEqual(
  buildYearBounds([ev("battle", 1805), ev("battle", 1815)]),
  { min: 1805, max: 1815 },
);

assert.deepEqual(
  buildYearBounds([
    ev("death", 1821),
    ev("birth", 1769, { start_time: null, time: { kind: "exact", start: "1769-08-15", precision: "day" } }),
  ]),
  { min: 1769, max: 1821 },
);

const stacked = spreadStackedMapPoints({
  type: "FeatureCollection",
  features: [0, 1, 2].map((i) => ({
    type: "Feature",
    id: String(i),
    geometry: { type: "Point", coordinates: [2, 47] },
    properties: { id: String(i) },
  })),
});
const coords = stacked.features.map((f) => f.geometry.coordinates);
assert.equal(new Set(coords.map(([x, y]) => `${x.toFixed(4)},${y.toFixed(4)}`)).size, 3);

const box = boundsOfMapFeatures({
  type: "FeatureCollection",
  features: [
    { type: "Feature", geometry: { type: "Point", coordinates: [-74, 40.7] }, properties: {} },
    { type: "Feature", geometry: { type: "Point", coordinates: [2.3, 48.8] }, properties: {} },
  ],
});
assert.ok(box);
assert.equal(box[0][0], -74);
assert.equal(box[1][0], 2.3);

console.log("geo.assert: ok");

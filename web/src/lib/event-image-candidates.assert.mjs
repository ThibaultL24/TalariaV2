// web/src/lib/event-image-candidates.assert.mjs
import assert from "node:assert/strict";
import { buildEventImageCandidates } from "./event-image-candidates.ts";

const battle = buildEventImageCandidates({
  eventType: "battle",
  personLabel: "Napoleon",
  placeLabel: "Waterloo",
  sourcePageTitles: ["Napoleon", "Battle of Waterloo"],
});
assert.equal(battle[0].title, "Battle of Waterloo");
assert.equal(battle[0].kind, "event");
assert.ok(battle.some((c) => c.kind === "place" && c.title === "Waterloo"));
assert.ok(!battle.some((c) => c.kind === "person"));

const birth = buildEventImageCandidates({
  eventType: "birth",
  personLabel: "Napoleon",
  placeLabel: "Ajaccio",
});
assert.ok(birth.some((c) => c.kind === "person"));

console.log("event-image-candidates.assert: ok");

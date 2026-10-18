// web/src/features/poreia/heritage-catalog.test.ts
import { expect, test } from "vitest";
import type { TimelineEvent } from "@/lib/api";
import type { VisitSite } from "@/lib/entity-views";
import { buildHeritageCatalog } from "./heritage-catalog";

const site: VisitSite = {
  id: "site-one",
  wikidata_qid: "Q123",
  label: "Maison historique",
  kind: "residence",
  relation_pid: "P551",
  coordinates: { lat: 48.8, lon: 2.3 },
  visitability: "unknown",
};

function event(partial: Partial<TimelineEvent> & { id: string; title: string }): TimelineEvent {
  return {
    entity_id: "person-one",
    event_type: "museum",
    epistemic_status: "verified",
    person: "Test",
    map_eligible: true,
    ...partial,
  };
}

test("a Wikidata site and a different historical place both stay visible", () => {
  const places = buildHeritageCatalog(
    [site],
    [event({ id: "heritage-one", title: "Monument documenté", coordinates: { lat: 48.8, lon: 2.3 } })],
  );
  expect(places.map((place) => place.title)).toEqual(["Maison historique", "Monument documenté"]);
});

test("the same Wikidata QID reinforces one place instead of creating a second row", () => {
  const places = buildHeritageCatalog(
    [site],
    [event({
      id: "heritage-one",
      title: "Maison historique",
      place_label: "Paris",
      wikidata_qid: "Q123",
    } as TimelineEvent & { wikidata_qid: string })],
  );
  expect(places).toHaveLength(1);
  expect(places[0].origins).toEqual(["wikidata_site", "heritage_event"]);
  expect(places[0].city).toBe("Paris");
  expect(places[0].sources[0]?.url).toContain("Q123");
});

test("verified hours are kept and missing hours are not invented", () => {
  const places = buildHeritageCatalog([{ ...site, opening_hours: "Mo-Su 10:00-18:00" }], []);
  expect(places[0].openingHours).toBe("Mo-Su 10:00-18:00");
  expect(buildHeritageCatalog([site], [])[0].openingHours).toBeUndefined();
});

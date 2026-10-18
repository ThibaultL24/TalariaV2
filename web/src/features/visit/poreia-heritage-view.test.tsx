import { afterEach, expect, test } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { TimelineEvent } from "@/lib/api";
import type { VisitSite } from "@/lib/entity-views";
import { ExperienceNavigation } from "@/components/layout/experience-navigation";
import { PoreiaHeritageView } from "./poreia-heritage-view";

afterEach(cleanup);

const site: VisitSite = {
  id: "site-one",
  wikidata_qid: "Q123",
  label: "Maison historique",
  kind: "residence",
  relation_pid: "P551",
  coordinates: {lat: 48.8, lon: 2.3},
  visitability: "unknown",
};

function renderHeritage(
  sites: VisitSite[],
  events: TimelineEvent[],
  loading = false,
) {
  const selected: string[] = [];
  render(
    <MemoryRouter initialEntries={["/poreia/person-one/heritage"]}>
      <PoreiaHeritageView
        entityId="person-one"
        sites={sites}
        events={events}
        loading={loading}
        onFocusSite={(item) => selected.push(item.id)}
        onFocusEvent={(id) => selected.push(id)}
      />
    </MemoryRouter>,
  );
  return selected;
}

test("Poreia shows documented sites without contradictory legacy empty text", () => {
  const selected = renderHeritage([site], []);
  expect(screen.getByText("Maison historique")).toBeTruthy();
  expect(screen.queryByText(/Aucun lieu visitable indexé|No visit places indexed/)).toBeNull();
  fireEvent.click(screen.getByRole("button", {name: /Maison historique/}));
  expect(selected).toEqual(["site-one"]);
});

test("Poreia shows one useful empty state when no site or heritage event exists", () => {
  renderHeritage([], []);
  expect(screen.getAllByRole("status")).toHaveLength(1);
  const link = screen.getByRole("link", {name: /Istoria/});
  expect(link.getAttribute("href")).toBe("/istoria/person-one/map");
  expect(screen.queryByText(/Aucun lieu visitable indexé|No visit places indexed/)).toBeNull();
});

test("Poreia keeps its legacy heritage data as a fallback", () => {
  const event = {
    id: "heritage-one",
    entity_id: "person-one",
    event_type: "museum",
    title: "Monument documenté",
    time: {kind:"unknown", precision:"unknown"},
  } as unknown as TimelineEvent;
  const selected = renderHeritage([], [event]);
  expect(screen.getByText("Monument documenté")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", {name: /Monument documenté/}));
  expect(selected).toEqual(["heritage-one"]);
});

test("experience navigation keeps three routes and preserves subject UUID", () => {
  render(
    <MemoryRouter initialEntries={["/poreia/person-one/heritage"]}>
      <ExperienceNavigation entityId="person-one" />
    </MemoryRouter>,
  );
  const nav = screen.getByRole("navigation", {name: /univers|experiences/});
  const links = [...nav.querySelectorAll("a")];
  expect(links).toHaveLength(3);
  expect(links.map(link => link.getAttribute("href"))).toEqual([
    "/istoria/person-one/overview", "/poreia/person-one/map", "/agora/person-one/discover",
  ]);
  expect(links[1].getAttribute("aria-current")).toBe("page");
  expect(links[0].getAttribute("aria-current")).toBeNull();
});

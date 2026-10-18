// web/src/features/experience-routing.test.tsx
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes, useParams, useSearchParams } from "react-router-dom";
import { LegacyEntityRedirect } from "@/app";
import { IntuitionProviders } from "@/lib/intuition/providers";
import { intuitionQueryClient } from "@/lib/intuition/clients";
import { legacyEntityTarget } from "@/lib/experiences";
import { useExplorerStore } from "@/stores/explorer-store";
import { IstoriaPage } from "@/features/istoria/pages/istoria-page";
import { PoreiaPage } from "@/features/poreia/pages/poreia-page";
import { AgoraPage } from "@/features/agora/pages/agora-page";

vi.mock("@/features/entity/entity-map", () => ({
  EntityMap: () => <div>Map surface</div>,
}));
vi.mock("@/components/timeline/historical-timeline", () => ({
  HistoricalTimeline: () => <div>Canvas surface</div>,
}));
vi.mock("@/features/visit/entity-visit-map", () => ({
  EntityVisitMap: () => <div>Visit map</div>,
}));
vi.mock("@/components/detail/event-detail-card", () => ({
  EventDetailCard: () => <div>Detail</div>,
}));

beforeEach(() => {
  useExplorerStore.getState().clearEntity();
  intuitionQueryClient.clear();
  vi.stubGlobal("fetch", vi.fn(async (input: string) => {
    const url = String(input);
    const body = url.includes("/overview")
      ? {
          entity: { id: "person-1", label: "Victor Hugo", qid: "Q535" },
          stats: { timeline_events: 2, mapped_events: 1, places: 1 },
          time_bounds: { from: 1802, to: 1885 },
        }
      : url.includes("/visit/sites")
        ? { sites: [], count: 0 }
        : url.includes("/claims")
          ? []
          : url.includes("/bibliography")
            ? { items: [], providers: [], next_cursor: null }
            : { events: [], count: 0, pagination: { next_cursor: null }, items: [] };
    return { ok: true, json: async () => body };
  }));
});

afterEach(() => {
  cleanup();
  intuitionQueryClient.clear();
  vi.unstubAllGlobals();
});

function renderAt(url: string) {
  render(
    <IntuitionProviders>
      <MemoryRouter initialEntries={[url]}>
        <Routes>
          <Route path="/istoria/:entityId/:view" element={<IstoriaPage />} />
          <Route path="/poreia/:entityId/:view" element={<PoreiaPage />} />
          <Route path="/entities/:entityId/:view" element={<LegacyEntityRedirect />} />
        </Routes>
      </MemoryRouter>
    </IntuitionProviders>,
  );
}

test("legacy visit URL keeps the person and the event, and drops the lens", () => {
  expect(legacyEntityTarget("person-1", "timeline", "?lens=visit&event=event-1")).toBe(
    "/poreia/person-1/map?event=event-1",
  );
  expect(legacyEntityTarget("person-1", "sources", "?from=1800")).toBe(
    "/istoria/person-1/sources?from=1800",
  );
});

test("legacy route redirects a visit lens onto Poreia", async () => {
  function Landed() {
    const { entityId, view } = useParams();
    const [params] = useSearchParams();
    return <p>{`${view}:${entityId}:${params.get("event") ?? ""}:${params.get("lens") ?? ""}`}</p>;
  }
  render(
    <MemoryRouter initialEntries={["/entities/person-1/timeline?lens=visit&event=event-1"]}>
      <Routes>
        <Route path="/entities/:entityId/:view" element={<LegacyEntityRedirect />} />
        <Route path="/poreia/:entityId/:view" element={<Landed />} />
      </Routes>
    </MemoryRouter>,
  );
  expect(await screen.findByText("map:person-1:event-1:")).toBeTruthy();
});

test("Istoria keeps the same person when opening Poreia", async () => {
  renderAt("/istoria/person-1/overview");
  await screen.findByRole("heading", { name: "Victor Hugo" });
  const poreia = screen.getAllByRole("link").find((link) => link.getAttribute("href") === "/poreia/person-1/map"
    || link.getAttribute("href") === "/poreia/person-1");
  expect(poreia).toBeTruthy();
  expect(screen.getByRole("link", { name: "Discover their places in Poreia →" }).getAttribute("href")).toBe(
    "/poreia/person-1/map",
  );
});

test("Poreia map does not request the canonical timeline or the heritage list", async () => {
  renderAt("/poreia/person-1/map");
  expect(await screen.findByText("Visit map")).toBeTruthy();
  const urls = vi.mocked(fetch).mock.calls.map(([url]) => String(url));
  expect(urls.some((url) => url.includes("/timeline"))).toBe(false);
  expect(urls.some((url) => url.includes("/visit/heritage") && !url.includes("geojson"))).toBe(false);
  expect(urls.some((url) => url.includes("/overview"))).toBe(true);
});

test("Agora deep link keeps the person and exposes one section nav", async () => {
  render(
    <IntuitionProviders>
      <MemoryRouter initialEntries={["/agora/person-1"]}>
        <Routes>
          <Route path="/agora/:entityId" element={<AgoraPage />} />
          <Route path="/agora/:entityId/:view" element={<AgoraPage />} />
          <Route path="/istoria/:entityId/:view" element={<p>Istoria</p>} />
        </Routes>
      </MemoryRouter>
    </IntuitionProviders>,
  );
  const theses = await screen.findByRole("link", { name: "Theses" });
  expect(theses.getAttribute("href")).toBe("/agora/person-1/theses");
  expect(screen.getByRole("link", { name: "Discover" }).getAttribute("aria-current")).toBe("page");
  expect(screen.queryAllByRole("tablist")).toHaveLength(0);
});

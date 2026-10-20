import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import {
  IngestionObservatory,
  IngestionJobDetail,
} from "./ingestion-observatory";
import type { Detail, Job, Overview } from "./ingestion-api";
const job: Job = {
  id: "1",
  lane: "agora",
  subject: "Victor Hugo",
  qid: "Q535",
  entity_id: null,
  status: "running",
  attempts: 2,
  resumes: 1,
  lease_expired: false,
  created_at: "2026-01-02T00:00:00Z",
  started_at: null,
  finished_at: null,
  duration_ms: null,
};
const overview: Overview = {
  summary: {
    queued: 1,
    running: 1,
    done: 2,
    failed: 1,
    resumes: 1,
    expired_leases: 0,
    mean_ms: 1000,
    p50_ms: 1000,
    p95_ms: 2000,
    success_percent: 66.7,
    failure_percent: 33.3,
    documents_persisted_unique: 3,
    documents_matched_unique: 2,
    instrumented_agora_jobs: 1,
  },
  volume: [{ day: "2026-01-02", jobs: 5 }],
};
const detail: Detail = {
  job,
  checkpoints: { items: [], total: 0, limit: 25, offset: 0 },
  metrics: {
    coverage: {
      documents_persisted_unique: 3,
      documents_matched_unique: 2,
      match_rate_percent: 66.7,
    },
    pagination: { total: 2, limit: 25, offset: 0, next_offset: null },
    provider_attempts: [1, 2].map((attempt) => ({
      provider: "hal",
      attempt,
      status: attempt === 1 ? "partial" : "healthy",
      error_kind: attempt === 1 ? "timeout" : null,
      discover_calls: 1,
      fetch_calls: 2,
      documents_discovered: 2,
      documents_persisted: 1,
      documents_skipped: 0,
      match_operations: 1,
      fetch_timeouts: attempt === 1 ? 1 : 0,
      connector_errors: 0,
      elapsed_ms: 1000,
      started_at: "2026-01-02T00:00:00Z",
      finished_at: "2026-01-02T00:00:01Z",
      interrupted: false,
    })),
  },
};
function json(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json" },
  });
}
function mockApi() {
  const fetcher = vi.fn(async (input: RequestInfo | URL) => {
    const path = String(input);
    if (path.includes("jobs/1")) return json(detail);
    if (path.includes("overview")) return json(overview);
    if (path.includes("providers"))
      return json({ items: [], total: 0, limit: 25, offset: 0 });
    return json({ items: [job], total: 30, limit: 25, offset: 0 });
  });
  vi.stubGlobal("fetch", fetcher);
  return fetcher;
}
const clients: QueryClient[] = [];
function mount(path = "/admin/ingestion") {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  clients.push(client);
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path="/admin/ingestion" element={<IngestionObservatory />} />
          <Route
            path="/admin/ingestion/jobs/:jobId"
            element={<IngestionJobDetail />}
          />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
  return client;
}
afterEach(() => {
  cleanup();
  clients.splice(0).forEach((c) => c.clear());
  vi.unstubAllGlobals();
});
describe("ingestion administration", () => {
  it("loads backend data, changes lane, paginates and navigates to multiple attempts", async () => {
    const api = mockApi();
    mount();
    expect(screen.getByText("Chargement du dashboard…")).toBeTruthy();
    const link = await screen.findByText("Victor Hugo · Q535");
    expect(screen.getByText("Volume quotidien (UTC)")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Lane"), {
      target: { value: "agora" },
    });
    fireEvent.click(screen.getByText("Appliquer les filtres"));
    await waitFor(() =>
      expect(
        api.mock.calls.some(([url]) => String(url).includes("lane=agora")),
      ).toBe(true),
    );
    fireEvent.click(screen.getAllByText("Suivant")[0]);
    await waitFor(() =>
      expect(
        api.mock.calls.some(([url]) => String(url).includes("offset=25")),
      ).toBe(true),
    );
    fireEvent.click(link);
    await screen.findByText(
      "Historique des tentatives, erreurs et interruptions",
    );
    expect(screen.getAllByText("hal")).toHaveLength(2);
    expect(screen.getByText("timeout")).toBeTruthy();
  });
  it.each([401, 403])(
    "denies access for HTTP %s without fetching secondary data",
    async (status) => {
      const api = vi.fn(async () => json({}, status));
      vi.stubGlobal("fetch", api);
      mount();
      await screen.findByRole("alert");
      expect(api).toHaveBeenCalledTimes(1);
      expect(screen.queryByText("Vue générale")).toBeNull();
    },
  );
  it("renders empty results", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input) =>
        String(input).includes("overview")
          ? json({ ...overview, volume: [] })
          : json({ items: [], total: 0, limit: 25, offset: 0 }),
      ),
    );
    mount();
    await screen.findByText("Aucun job correspondant.");
    expect(
      screen.getByText("Aucune ingestion sur cette période."),
    ).toBeTruthy();
  });
  it("keeps displayed results during refresh and reports API errors", async () => {
    const api = mockApi();
    mount();
    await screen.findByText("Victor Hugo · Q535");
    let finish: ((r: Response) => void) | undefined;
    api.mockImplementation(async (input) =>
      String(input).includes("overview")
        ? new Promise<Response>((resolve) => {
            finish = resolve;
          })
        : String(input).includes("providers")
          ? json({ items: [], total: 0, limit: 25, offset: 0 })
          : json({ items: [job], total: 1, limit: 25, offset: 0 }),
    );
    fireEvent.click(screen.getByText("Actualiser"));
    await waitFor(() => expect(finish).toBeDefined());
    expect(screen.getByText("Vue générale")).toBeTruthy();
    expect(screen.getByText("Victor Hugo · Q535")).toBeTruthy();
    finish!(json({}, 500));
    await screen.findByRole("alert");
    expect(screen.getByText("Vue générale")).toBeTruthy();
  });
  it("removes private data when authorization is revoked during refresh", async () => {
    const api = mockApi();
    mount();
    await screen.findByText("Victor Hugo · Q535");
    api.mockImplementation(async () => json({}, 403));
    fireEvent.click(screen.getByText("Actualiser"));
    await screen.findByRole("alert");
    expect(screen.queryByText("Victor Hugo · Q535")).toBeNull();
  });
});

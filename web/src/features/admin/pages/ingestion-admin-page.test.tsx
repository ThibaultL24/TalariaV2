import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { IngestionAdminPage } from "./ingestion-admin-page";

vi.mock("@/components/layout/navbar", () => ({ Navbar: () => <nav>Talaria navbar</nav> }));

afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

const summary = {
  days: 30,
  coverage_meaning: "document_linkage_not_historical_truth",
  jobs: { total: 2, queued: 0, running: 1, done: 1, failed: 0, retried: 1, p50_ms: 1500, p95_ms: 2700 },
  providers: [
    { provider: "hal", attempts: 2, failed: 1, rate_limited: 1, timeouts: 0,
      fetch_calls: 6, persisted_operations: 2, p50_ms: 200, p95_ms: 800,
      documents_persisted_unique: 2, documents_matched_unique: 1, document_match_percent: 50 },
  ],
};
const jobs = {
  days: 30,
  items: [{
    job_id: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
    lane: "agora", subject: "Victor Hugo", qid: "Q535",
    status: "done", attempts: 2, duration_ms: 2700,
    created_at: "2026-10-20T10:00:00Z", started_at: "2026-10-20T10:00:01Z",
    finished_at: "2026-10-20T10:00:04Z",
  }],
  pagination: { limit: 25, offset: 0, total: 1, next_offset: null },
};
const details = {
  job_id: jobs.items[0].job_id, status: "done", attempt_count: 2,
  coverage: {
    meaning: "entity_document_match_not_historical_verification",
    documents_persisted_unique: 2, documents_matched_unique: 1, match_rate_percent: 50,
  },
  provider_attempts: [{
    provider: "hal", attempt: 1, status: "rate_limited", error_kind: "rate_limit",
    fetch_calls: 6, documents_discovered: 10, documents_persisted: 2,
    fetch_timeouts: 0, connector_errors: 1, elapsed_ms: 800, interrupted: false,
  }],
  pagination: { limit: 100, offset: 0, total: 1, next_offset: null },
};
function open() {
  return render(<MemoryRouter><IngestionAdminPage /></MemoryRouter>);
}

test("summarizes real server ratios and allows inspecting one job", async () => {
  const fetchMock = vi.fn(async (path: string) => ({
    ok: true,
    json: async () => path.includes("/summary") ? summary
      : path.includes("/metrics?") ? details : jobs,
  }));
  vi.stubGlobal("fetch", fetchMock);
  open();
  expect(await screen.findByRole("heading", { name: "Observatoire des ingestions" })).toBeTruthy();
  expect(await screen.findByText("Victor Hugo")).toBeTruthy();
  expect(screen.getByText("50 %")).toBeTruthy();
  expect(screen.getByText("HAL", { exact: false })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Inspecter" }));
  expect(await screen.findByRole("heading", { name: "Diagnostic du job" })).toBeTruthy();
  expect(await screen.findByText("rate_limit")).toBeTruthy();
  expect(fetchMock.mock.calls.some(([url]) => String(url).includes("/metrics?"))).toBe(true);
});

test("refuses to render data when the server returns forbidden", async () => {
  vi.stubGlobal("fetch", vi.fn(async () => ({ ok: false, status: 403 })));
  open();
  expect(await screen.findByRole("alert")).toBeTruthy();
  expect(screen.getByText(/droits d'administration/)).toBeTruthy();
  expect(screen.queryByText("Historique des jobs")).toBeNull();
});

test("server filters receive the selected lane and period", async () => {
  const fetchMock = vi.fn(async (path: string) => ({
    ok: true, json: async () => path.includes("/summary") ? summary : jobs,
  }));
  vi.stubGlobal("fetch", fetchMock);
  open();
  await screen.findByText("Victor Hugo");
  fireEvent.change(screen.getByLabelText("Expérience"), { target: { value: "agora" } });
  await waitFor(() => expect(fetchMock.mock.calls.some(([url]) =>
    String(url).includes("lane=agora"))).toBe(true));
});

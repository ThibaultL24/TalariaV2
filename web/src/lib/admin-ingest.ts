// Read-only admin contract. Server is authoritative for permissions and ratios.
export interface IngestJobStats {
  total: number;
  queued: number;
  running: number;
  done: number;
  failed: number;
  retried: number;
  p50_ms: number | null;
  p95_ms: number | null;
}
export interface IngestProviderStats {
  provider: string;
  attempts: number;
  failed: number;
  rate_limited: number;
  timeouts: number;
  fetch_calls: number;
  persisted_operations: number;
  p50_ms: number | null;
  p95_ms: number | null;
  documents_persisted_unique: number;
  documents_matched_unique: number;
  document_match_percent: number | null;
}
export interface AdminIngestionSummary {
  days: number;
  jobs: IngestJobStats;
  coverage_meaning: "document_linkage_not_historical_truth";
  providers: IngestProviderStats[];
}
export interface AdminIngestJob {
  job_id: string;
  lane: "agora" | "explorer";
  subject: string;
  qid: string | null;
  status: "queued" | "running" | "done" | "failed";
  attempts: number;
  duration_ms: number | null;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
}
export interface AdminJobPage {
  days: number;
  items: AdminIngestJob[];
  pagination: { limit: number; offset: number; total: number; next_offset: number | null };
}
export interface AdminProviderAttempt {
  provider: string;
  attempt: number;
  status: string;
  error_kind: string | null;
  fetch_calls: number;
  documents_discovered: number;
  documents_persisted: number;
  fetch_timeouts: number;
  connector_errors: number;
  elapsed_ms: number;
  interrupted: boolean;
}
export interface AdminJobMetrics {
  job_id: string;
  status: string;
  attempt_count: number;
  coverage: {
    meaning: string;
    documents_persisted_unique: number;
    documents_matched_unique: number;
    match_rate_percent: number | null;
  };
  provider_attempts: AdminProviderAttempt[];
  pagination: { limit: number; offset: number; total: number; next_offset: number | null };
}

export class AdminApiError extends Error {
  constructor(public readonly status: number) {
    super(status === 401 ? "Connectez votre portefeuille Talaria pour accéder à cette page."
      : status === 403 ? "Ce compte ne dispose pas des droits d'administration."
      : "Impossible de charger les diagnostics d'ingestion.");
    this.name = "AdminApiError";
  }
}
async function getAdmin<T>(path: string, signal?: AbortSignal): Promise<T> {
  const response = await fetch(path, { signal, credentials: "same-origin" });
  if (!response.ok) throw new AdminApiError(response.status);
  return response.json() as Promise<T>;
}
export function fetchAdminSummary(days: number, signal?: AbortSignal) {
  return getAdmin<AdminIngestionSummary>(
    `/api/v1/admin/ingestions/summary?days=${days}`, signal);
}
export function fetchAdminJobs(days: number, lane: string, status: string,
  offset: number, signal?: AbortSignal) {
  const params = new URLSearchParams({ days: String(days), limit: "25", offset: String(offset) });
  if (lane) params.set("lane", lane);
  if (status) params.set("status", status);
  return getAdmin<AdminJobPage>(
    `/api/v1/admin/ingestions/jobs?${params}`, signal);
}
export function fetchAdminJobMetrics(id: string, signal?: AbortSignal) {
  return getAdmin<AdminJobMetrics>(
    `/api/v1/ingest/${encodeURIComponent(id)}/metrics?limit=100&offset=0`, signal);
}

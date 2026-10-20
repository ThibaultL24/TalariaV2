export type Lane = "explorer" | "agora";
export type Status = "queued" | "running" | "done" | "failed";
export interface Job {
  id: string;
  lane: Lane;
  subject: string;
  qid: string | null;
  entity_id: string | null;
  status: Status;
  attempts: number;
  resumes: number;
  lease_expired: boolean;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  duration_ms: number | null;
}
export interface Page<T> {
  items: T[];
  total: number;
  limit: number;
  offset: number;
}
export interface Overview {
  summary: {
    queued: number;
    running: number;
    done: number;
    failed: number;
    resumes: number;
    expired_leases: number;
    mean_ms: number | null;
    p50_ms: number | null;
    p95_ms: number | null;
    success_percent: number | null;
    failure_percent: number | null;
    documents_persisted_unique: number;
    documents_matched_unique: number;
    instrumented_agora_jobs: number;
  };
  volume: { day: string; jobs: number }[];
}
export interface Provider {
  provider: string;
  attempts: number;
  completed_attempts: number;
  mean_ms: number | null;
  p50_ms: number | null;
  p95_ms: number | null;
  elapsed_ms: number;
  discover_calls: number;
  fetch_calls: number;
  discovered: number;
  persisted: number;
  timeouts: number;
  errors: number;
  rate_limited_attempts: number;
  resumes: number;
  interrupted_attempts: number;
  failure_percent: number | null;
  documents_persisted_unique: number;
  documents_matched_unique: number;
  match_rate_percent: number | null;
}
export interface Attempt {
  provider: string;
  attempt: number;
  status: string;
  error_kind: string | null;
  discover_calls: number;
  fetch_calls: number;
  documents_discovered: number;
  documents_persisted: number;
  documents_skipped: number;
  match_operations: number;
  fetch_timeouts: number;
  connector_errors: number;
  elapsed_ms: number;
  started_at: string;
  finished_at: string | null;
  interrupted: boolean;
}
export interface Detail {
  job: Job;
  checkpoints: Page<{
    source_kind: string;
    connector_version: string;
    completed_at: string;
  }>;
  metrics: {
    provider_attempts: Attempt[];
    pagination: {
      total: number;
      limit: number;
      offset: number;
      next_offset: number | null;
    };
    coverage: {
      documents_persisted_unique: number;
      documents_matched_unique: number;
      match_rate_percent: number | null;
    };
  };
}
export class AdminError extends Error {
  constructor(public status: number) {
    super(
      status === 401
        ? "Connexion requise avec votre session Talaria."
        : status === 403
          ? "Accès refusé : un rôle administrateur est requis."
          : "Impossible de charger les données d’administration.",
    );
  }
}
export async function adminGet<T>(
  path: string,
  signal?: AbortSignal,
): Promise<T> {
  const response = await fetch(`/api/v1/admin/ingestion/${path}`, {
    credentials: "same-origin",
    cache: "no-store",
    signal,
  });
  if (!response.ok) throw new AdminError(response.status);
  return response.json() as Promise<T>;
}

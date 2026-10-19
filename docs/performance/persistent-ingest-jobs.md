# Persistent ingestion job worker — production contract

## Scope
API routes `POST /api/v1/ingest/explorer`, `POST /api/v1/ingest/agora`,
`GET /api/v1/ingest/{job_id}`, and `GET /api/v1/ingest/explorer/{job_id}/status`
now use PostgreSQL table `ingest_jobs` as the **source of truth** for admission,
progress snapshots, terminal results and cross-replica state.

This does **not** change the person pipeline, canonical append-only facts,
source evidence or corpus-to-claim separation. Job payloads are typed JSON
serializations of the existing `StartIngestBody`. Existing API URLs remain unchanged.

## Flow
1. Validate the HTTP request. If the same lane and identity already has a queued
   or running row, return that job's UUID and `deduped: true`.
2. Insert with `ON CONFLICT DO NOTHING`; a **partial unique index** on active
   `(lane, identity_key)` ensures only one job is admitted across API replicas.
   A QID, if supplied, defines identity; otherwise normalized name is used.
3. Each API replica polls the queue. PostgreSQL `FOR UPDATE SKIP LOCKED` claims
   at most one row per worker, sets a random owner token and 90-second lease.
4. During processing the worker heartbeats every 15 seconds, saving typed
   progress and entity ID. Lease loss or heartbeat failure **cancels** the worker.
5. A crashed process leaves a running row. After lease expiry, another worker
   claims the row, reconstructs the original typed request and re-executes its
   idempotent ingestion from the beginning. Retry count is bounded to **3**.
   Jobs that exhaust retries are marked `failed`.
6. `finish` only updates a row whose owner token and lease are still valid.
   Completed and failed jobs retain their reports for later polling.

## Configuration
`TALARIA_INGEST_WORKERS=1` (default; range 1–3): maximum simultaneous jobs
**per API replica**. When `offline_only` is active the worker is disabled.
Run PostgreSQL migration `044_ingest_jobs.sql` before deploying the new binary.
Retain the current storage connection pool and progress polling endpoints.

## Performance and observability
- Index: unique `ingest_jobs_active_identity` protects concurrent admission;
  `ingest_jobs_due` enables worker claims; `ingest_jobs_entity_history`
  supports future per-person diagnostics.
- Worker: 2-second empty queue poll, 5-second DB-error retry, 15-second heartbeat,
  90-second lease. Consider per-provider quotas *separately* from job slots.
- Measure queue latency (`started_at-created_at`), job duration, number of retries,
  lease expirations, and throughput by lane.
- Query example:
  `SELECT lane,status,count(*),avg(extract(epoch FROM (coalesce(finished_at,now())-created_at))) AS seconds FROM ingest_jobs GROUP BY lane,status;`

## Recovery and safety caveats
- This is **at-least-once execution**, not an exactly-once distributed transaction.
  A process crash may force reprocessing of an incomplete document or occurrence;
  the existing upsert/idempotency gates must remain active. Verify source-level
  idempotence before increasing concurrency or retry count.
- The 90-second lease protects concurrent worker ownership, not every in-flight
  external HTTP call. Cancellation occurs at heartbeat/check points.
- Retries are whole-job replay, **not cursor-level resume**.
- If migrations are incompatible or the DB becomes unavailable, API admission fails
  rather than falling back to a potentially duplicated in-memory queue.
- Maintain a recovery runbook: inspect stale leases, attempts and the last error,
  then retry a terminal job by issuing a new HTTP ingest request. Never mutate
  canonical events directly to recover a job.

## Validation
SQL integration tests cover concurrent admission, distinct homonyms across
lanes, lock ownership, stale-lease reclamation, terminal records and retries.
The existing frontend status contract remains stable; CI runs `talaria-store`
ingestion tests, `talaria-api` route tests and Rust Clippy.

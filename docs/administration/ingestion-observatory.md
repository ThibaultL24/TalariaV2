# Ingestion Observatory

The read-only dashboard is at `/admin/ingestion`; individual jobs are at
`/admin/ingestion/jobs/<uuid>`. It uses real PostgreSQL telemetry, existing SIWE
sessions and TanStack Query. No ingestion execution or historical data mutation
is performed by these routes.

## Authentication and rollout

Migration **047_ingestion_observatory.sql** adds `administration_users` and the
`ingest_jobs_created_id (created_at DESC, id DESC)` index. No account receives
administrator access automatically. Existing session identities are reused;
there is no admin password, browser token or parallel login system.

An operator with database access grants a verified existing user explicitly:

```sql
INSERT INTO administration_users(user_id)
VALUES ('<verified-users.id>'::uuid)
ON CONFLICT DO NOTHING;
-- Revoke; effective on the next request, including background refresh:
DELETE FROM administration_users WHERE user_id = '<verified-users.id>'::uuid;
```

Resolve `users.id` from the existing verified `user_identities` wallet binding,
not from a browser-supplied role or an unverified display name. Grants are not
available over HTTP. Sign in through the existing Talaria wallet flow, then open
the administration URL. Missing/revoked/expired sessions receive 401, authenticated
users without a grant receive 403, and database errors receive 500. Every
administrative response has `Cache-Control: no-store`. Authorization middleware
runs before query/path extraction. The UI removes displayed private data if a
refresh receives 401/403 and does not retry these responses.

Before rollout, rebuild the API so SQLx embeds 047, apply migrations through the
normal deployment mechanism, and verify no grants exist before explicit operator
provisioning. The accessible development database was at 043 during this work;
044–047 were exercised through isolated SQLx databases, not applied to production.
The ordinary migration creates one index and can take a lock: schedule rollout
according to the size/write traffic of `ingest_jobs`. No Render deployment is
part of this change.

## HTTP contract

All routes below require an authorized `talaria_session` cookie:

| GET route | Response |
|---|---|
| `/api/v1/admin/ingestion/overview` | `{summary, volume}` |
| `/api/v1/admin/ingestion/jobs` | `{items, total, limit, offset}` |
| `/api/v1/admin/ingestion/providers` | `{items, total, limit, offset}` |
| `/api/v1/admin/ingestion/jobs/{job_id}` | `{job, metrics, checkpoints}` |
| `/api/v1/ingest/{job_id}/metrics` | Existing diagnostic shape, now admin-only |

The public `/api/v1/ingest/{job_id}` and Explorer status endpoints retain
progress, identity and map/timeline counts. `error` is now a generic
`ingestion_failed` code, and the former raw `report` field is `null`.
No frontend consumer in main used that raw report. Diagnostic clients must now
use an authorized existing session; knowing a UUID is not authorization.

Example query:

```text
GET /api/v1/admin/ingestion/jobs?from=2026-10-01T00:00:00Z&until=2026-10-21T00:00:00Z&lane=agora&qid=Q517&limit=25&offset=0
```

Empty overview (illustrative contract, not production measurements):

```json
{
  "summary": {
    "queued": 0, "running": 0, "done": 0, "failed": 0,
    "resumes": 0, "expired_leases": 0,
    "mean_ms": null, "p50_ms": null, "p95_ms": null,
    "success_percent": null, "failure_percent": null,
    "documents_persisted_unique": 0, "documents_matched_unique": 0,
    "instrumented_agora_jobs": 0
  },
  "volume": []
}
```

Empty job/provider page: `{"items":[],"total":0,"limit":25,"offset":0}`.
Errors use safe error codes; Axum extraction failures can return text with 400.
Full response schemas and error statuses are in both synchronized OpenAPI files.

### Filters and populations

- Cohort: **jobs created in `[from, until)`**, RFC3339 with timezone. Defaults are
  the last seven days; maximum span 93 days. UI inputs are explicitly UTC.
- `lane`: `explorer` (Istoria), `agora`, or omitted for both.
- `status`: current `queued`, `running`, `done`, `failed`.
- `qid`: exact QID; identities are never merged by display label.
- `subject`: case-insensitive exact name; prefer QID for homonyms.
- `provider`: jobs with an observed attempt for that provider. Provider counters
  and coverage are restricted to that provider, while job durations still
  describe the whole job.
- `limit`: 1–100, default 25 (50 for diagnostics); `offset`: 0–10000.
- Provider `sort`: `slowest` (p95), `calls` (discovery + fetch), `errors` (failure
  rate), with provider-name tie-break. These measure observed work, not monetary
  cost. The API accepts `sort` on cohort routes but only providers uses it.
- Unknown parameters, invalid lanes/statuses/QIDs, reversed periods and excess
  pagination are rejected. Historical `pipeline=person` remains unchanged.

Job mean/p50/p95 include done and failed jobs with a measured start/end, from
**first start to final finish**, including retry backoff. They exclude queue time.
Success/failure rates use all terminal jobs as denominator. Provider duration
percentiles use finished attempts, not running/incomplete observations. Provider
failure rate counts partial, failed and rate_limited among configured terminal
attempts; not_configured is excluded from that denominator. Percentiles use
PostgreSQL `percentile_cont`; empty statistical populations produce null.

Job resumes are `max(attempts - 1, 0)`. A provider resume requires an earlier
record for the same job/provider; a provider first visited during job attempt 2
is not a provider resume. Expired leases are the current snapshot, not a
historical count. An unfinished provider attempt is inferred interrupted if the
job advanced to another attempt, is no longer running, or its lease expired.

Documents discovered/persisted in attempts are **operations**. Documentary
coverage counts distinct `corpus_document_id`, independently of attempts and
provider aliases. A match requires a link to the job's **entity_id**. Multiple
relations and repeated ingestion cannot inflate unique counts. Global totals do
not sum provider buckets. Linkage reflects current database links, not a frozen
historical snapshot. It is never a confidence, truth or source reliability score.

## Architecture and performance

`talaria-store::ingestion_observatory` holds parameterized SQL projections;
`routes::admin_ingestion` holds validation, session authorization and HTTP
composition. The detail endpoint reuses existing metrics instead of returning
raw job JSON, connector messages or request payloads. Existing lease-fenced
telemetry writes are unchanged: write failures still propagate through the
existing worker recovery mechanism, never bypassing lease fencing.

Aggregations run in PostgreSQL, never across frontend pages. Count/page and
summary/volume pairs use read-only repeatable-read transactions. Administrative
cohort queries have a local 10-second statement timeout. The creation index
supports bounded period selection and stable descending pagination. Existing
job/provider primary keys and document/link indexes support joins; no redundant
provider counter index is added. SQLx tests execute `EXPLAIN (ANALYZE, BUFFERS)`
for a period-page query on a small fixture: PostgreSQL used `Index Only Scan`
on `ingest_jobs_created_id` (2 rows, 2 buffer hits, 0.040 ms execution). This is not a production-scale
benchmark; inspect real plans after deployment before adding more indexes.

The browser caches queries for 10 seconds, refreshes at 15-second intervals only
while visible, aborts abandoned requests, and preserves results during refresh.
The UI defaults to an upper bound at the next UTC midnight so newly admitted
jobs appear during the day; custom cohorts remain fixed. Terminal job detail
polling stops automatically. Errors keep the last successful data except access
revocation. No metrics are persisted to browser storage. Checkpoints and attempt
history are independently ordered but share the detail page offset/limit; both
provide totals, and an empty subpage is explicitly labeled.

## Verification and limits

SQLx tests apply the migration chain, check administrator access/revocation,
percentiles, repeated attempts, distinct documents, cross-provider aliases,
QID homonyms, empty periods, pagination, sensitive-field exclusion, and unchanged
canonical rows. React tests cover real API-shaped responses, filtering,
pagination, multiple attempts, navigation, empty/error states and refresh.
CI runs these tests and `scripts/check_observatory_contract.py`.

Remaining instrumentation work:

1. Istoria has durable job status/timing but **no per-provider attempt journal**.
   Its empty provider rows do not mean no source work occurred. Document coverage
   here is the Agora journal, not all historical sources or canonical evidence.
2. Older jobs without attempt observations cannot be reconstructed. The dashboard
   exposes instrumented Agora job count and does not invent missing telemetry.
3. Exact HTTP 429 count, discovery timeout count, request latency distributions,
   financial costs and exhaustive lease/heartbeat history are not recorded.
   Rate-limit counts represent classified attempts; timeouts are recorded fetch
   timeouts. Add typed observations before presenting finer metrics.
4. Periods wider than 93 days or offsets beyond 10000 require narrower filters;
   no unbounded export is offered.
5. Consider keyset pagination/rollups only after measuring production plans and
   volume. Large PostgreSQL aggregates can return a safe 500 on the 10s timeout.

## Local validation record

- Store: 46 tests passed; all migrations applied in temporary SQLx databases.
- API routes: 53 tests passed, including session and administration controls.
- Quality and sources: 427 tests passed. One pre-existing posthumous extraction
  test was updated to distinguish `memorial` from generic `commemoration`, without
  changing extraction or gate logic.
- Frontend: 121 tests passed with two workers; unrestricted workers saturated
  this machine and triggered existing timing-sensitive tests. CI now uses two.
- TypeScript and Vite build passed. Existing large bundle warnings remain.
- Cargo check and targeted Clippy passed with pre-existing warnings; no new
  observatory-specific Clippy warning.
- Both OpenAPI copies, schemas, security declarations and references validated.
  Previously frontend-only Agora source/argument/promote contracts are preserved
  and synchronized into the documentation copy.

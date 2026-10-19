# Corpus ingestion: bounded fetch pipeline

Applicable pipeline: **Agora bibliographic corpus**. Istoria's canonical person-events pipeline is unchanged.

## Execution model

The existing provider order and `provider_budget` reservation algorithm remain authoritative. Each provider discovers cursor pages; document metadata `fetch` calls run in bounded groups, while SQLx persistence (discovered row, normalization, corpus document, snapshot, entity link) runs **sequentially in discovery order**. No catalog metadata can create a canonical event or claim.

- `TALARIA_CORPUS_FETCH_CONCURRENCY`: fetch requests per provider, integer 1–4 (default 2). The limit is local to one active corpus run; concurrent runs are not globally capped.
- The existing `CorpusIngestLimits.provider_timeout` now applies to each **discover and fetch** request, separately (the Agora endpoint defaults to 30 seconds).
- On fetch errors, the document is recorded as skipped and a later discovered document may use its slot. After a 429/rate-limit error, the provider stops scheduling new requests, preserving partial results.
- A cursor that repeats exactly causes the provider to stop rather than loop indefinitely.
- Each discovery run now reports per-provider `fetch_calls`, `fetch_timeouts` and total `fetch_timeouts`, alongside previous counters.

## Limits and risk

Concurrence is **per run, per provider**, not a distributed rate limiter. Operators running multiple app replicas should use the default concurrency or set `TALARIA_CORPUS_FETCH_CONCURRENCY=1` for quota-sensitive providers. A separate durable job / lease mechanism is still required to ensure only one run per subject across instances.

This change does not increase the existing 15 documents per provider / 120 globally default API budgets. These remain configurable with `corpus_limit_per_provider`, `corpus_limit_total` and `minimum_per_provider`, while requests need explicit server-side bounds before wider public exposure.

## Benchmark protocol

Measure three cohorts (dense biography, sparse biography, source outages). Record `metrics.providers[provider].elapsed_ms`, `fetch_calls`, `fetch_timeouts`, `connector_errors` and `documents_persisted`. Compare concurrency 1 versus 2 on the same sample; keep the setting only if p95 latency improves without a corresponding increase in HTTP 429 or source errors.

## Tests and DB

- Unit tests exercise simultaneous fetch overlap, deterministic result order, timeouts, 429 handling and cursor validation.
- Existing PostgreSQL migrations / corpus provenance tables and keyset pagination are unchanged.
- No new DB index is required for this bounded network-I/O change. Retain unique `(source_kind, external_id)` behavior and snapshot idempotency.

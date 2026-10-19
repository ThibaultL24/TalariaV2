# Ingest job admission: single-instance duplicate suppression

Status: applies to both `explorer` and `agora` ingestion start endpoints.

The initial fast lookup for an active job avoids unnecessary QID/entity work on most repeat requests. The final lookup and insertion now occur while holding **one mutex**, so concurrent starts in the same Rust process cannot enqueue two active jobs for the same lane and identity.

When both sides have a Wikidata QID, that ID is authoritative; two people with the same name but different QIDs are **not** merged. Two subjects without QIDs may be deduplicated by normalized case-insensitive label within a lane. A request with a QID is not deduplicated against a name-only job, because identity equivalence is unproven.

No API changes: a repeated request returns the existing `job_id` and `deduped: true`. `queued` and `running` are active; completed and failed runs may be started again. Tests cover contention, lane isolation and same-name/different-QID identities.

**Outstanding production task:** the `IngestJobMap` is process-local. Cross-replica deduplication, durable progress, crash recovery and rate limiting need a persistent PostgreSQL job table, uniqueness constraints and worker leases. No claims are made that this change solves those cases.

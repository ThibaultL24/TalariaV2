# Progressive Istoria timeline

- Cache identity: `["istoria", "timeline", entity_id, filters, locale]`. Never reuse pages across people, language or filters.
- Loads 200 canonical events initially; additional pages require an explicit user action.
- Uses server `pagination.next_cursor` directly, keeping the Rust timeline API contract unchanged.
- Ingestion revisions invalidate cached pages and refetch without clearing the current display.
- Event detail is queried independently by its stable event ID, so a deep-link works even when the selected event is not in the first page.
- No additional PostgreSQL indexes are necessary for this change: the canonical person/timeline index and existing keyset cursor remain in use. Benchmark p50/p95, page count and transferred bytes before introducing another index.

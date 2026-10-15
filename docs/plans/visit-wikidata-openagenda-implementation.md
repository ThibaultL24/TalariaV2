# Visit: Wikidata sites, OSM hours and OpenAgenda

Branch: `cursor/visit-wikidata-openagenda`, based on `main` (`04acd58`).
Do not transplant migrations 033–035 from `cursor/local-aegean-tourism`: this branch uses the 033–040 sequence already present in `main`.

## Changes

- Migration `041_visit_wikidata_sites.sql`: independent `visit_sites` and `visit_person_sites`, QID dedup, source PID, spatial index, typed access status.
- `POST /api/v1/entities/{entity_id}/visit/discover`: Wikidata P547/P138 inverse and P119/P551 outbound; direct P625 and narrowly allowlisted heritage classes. Invalid and non-site results skipped. No proximity-only relationship, no canonical writes.
- `GET /api/v1/entities/{entity_id}/visit/sites` and `/visit/sites/geojson`: site-oriented API; `bbox` validation and result bound.
- `POST /api/v1/visit/sites/{site_id}/refresh-osm`: optional Overpass query for `opening_hours` **only when OSM wikidata tag exactly equals the site QID**. Exactly one matching object is required; its raw schedule is not proof the site is open now.
- Existing `OPENAGENDA_API_KEY` / `OPENAGENDA_PUBLIC_KEY` integration retained in `visit-enrich --live`; Unicode-safe request parameters and strict datetime/subject filter. Writes to `visit_opportunities`, never `canonical_events`. An absent key skips the provider.
- Visit map now merges separate documented sites with existing heritage and activity layers; site list exposed on `heritage`. Faits & anecdotes and Tourisme remain distinct.

## Known limits

- New Wikidata import is **explicit**, not scheduled. Mutation endpoints return 403 unless `TALARIA_ENABLE_VISIT_DISCOVERY=1` is set. This feature flag is not authentication: add authentication, quotas and cache coordination before enabling on public production.
- The query is intentionally conservative: some important sites will be absent if Wikidata lacks an eligible type or direct P625. No empty-site placeholder is created.
- No paid third-party service is required for Wikidata lookup; OpenAgenda requires a configured free public API key.
- `visit_opportunities` is a pre-existing `main` table, not the unrelated `cultural_activities` table from the divergent local branch.
- Existing `canonical_events` heritage rows from `main` remain available for compatibility, but the new Wikidata/fixture heritage writers have been disabled. No destructive migration of historical data was attempted.
- OSM enrichment is an explicit action; it does not infer visitability and is not run at request-time on normal site GETs.
- The current GraphQL/REST Wikidata revision ID is not returned by the direct-claim SPARQL query, so `wikidata_revision_id` remains nullable; revisit statement-level provenance before publishing data as independently corroborated.
- Do not restore the provided demo snapshot onto a DB with valuable local data. Snapshot restore is destructive.

## Local verification (not run in GitHub connector session)

```bash
git switch cursor/visit-wikidata-openagenda
# Use a dedicated empty Postgres database on port 5433.
# Apply ONLY the migration chain from this branch.
cargo test -p talaria-api --bin talaria visit_wikidata
cargo test -p talaria-store --lib
cargo check -p talaria-api
cd web && npm ci && npm run build && npx vitest run
```

For API smoke tests, obtain the internal UUID (not the QID):

```bash
TALARIA_ENABLE_VISIT_DISCOVERY=1 cargo run -p talaria-api -- serve # in API terminal\n# From another terminal:\ncurl -X POST http://localhost:8080/api/v1/entities/14eb944a-fa31-459a-be7d-1f32488e2bd6/visit/discover
curl http://localhost:8080/api/v1/entities/14eb944a-fa31-459a-be7d-1f32488e2bd6/visit/sites
curl http://localhost:8080/api/v1/entities/14eb944a-fa31-459a-be7d-1f32488e2bd6/visit/sites/geojson
```

### Database validation

Verify one site QID is not duplicated and repeated discovery preserves relationships:

```sql
SELECT wikidata_qid, COUNT(*) FROM visit_sites GROUP BY wikidata_qid HAVING COUNT(*) > 1;
SELECT entity_id, site_id, relation_pid, COUNT(*)
FROM visit_person_sites GROUP BY 1,2,3 HAVING COUNT(*) > 1;
```

Compare before/after counts of `canonical_events` when running `/visit/discover`: there must be **no change**. Also compare the Scholar timeline and `pipeline='person'` evidence joins.

## Release blockers

1. Run Rust/Postgres/Vitest/TypeScript tests in a proper clone; this connector environment cannot execute the repository tests.
2. Add database integration tests for repeated discovery and verify the number of accepted POIs on actual Wikidata payloads.
3. Add rate limiting and authorization to the discovery and OSM refresh endpoints before public deployment.
4. Decide on UI rendering for a site-specific detail pane, rather than routing site UUIDs to `/events/{id}`.
5. Check the provider API contract/live fixtures for OpenAgenda and Overpass from the deployment network.
6. Validate OSRM/Valhalla separately before adding travel-time estimates.

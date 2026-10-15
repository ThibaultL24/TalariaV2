# Istoria lead landmarks + Poreia destination-scoped sites

Branch based on `cursor/istoria-poreia-experience` (not `main` or `V3`). This branch follows the **main 033–040 + 041** migration lineage. **Never** combine with migrations 033–035 from `cursor/local-aegean-tourism`.

## Istoria: sourced, non-generative first-paragraph matching

- `042_istoria_landmark_mentions.sql` stores a replaceable derived index of links: existing canonical person event ↔ event evidence ↔ local Wikipedia lead sentence and wiki page revision.
- `POST /api/v1/entities/:id/landmarks/refresh` updates only this projection in one transaction. Returns 403 unless `TALARIA_ENABLE_LANDMARK_REFRESH=1`; keep disabled on public deployments. The feature flag is not authentication.
- `GET /api/v1/entities/:id/landmarks?limit=12` returns ranked matched events with source excerpt and localized Wikipedia revision URL, without running extraction/LLM.
- The matching is **conservative**: source sentence must be in the locally stored lead and already serve as evidence for the same active `pipeline='person'` canonical event. Lack of results is not treated as absence of important facts.
- Overview uses the sourced projection first, with a **visibly labeled heuristic fallback** when no matches exist. The older heuristic may still favor event types without proof that Wikipedia's lead highlights them.
- Importance still uses simple type scoring; **not** a verified biographical salience score. No new historical event, claim, place or evidence is inferred or persisted.

## Poreia: geographic destination filter

- `GET /api/v1/entities/:id/visit/itinerary?max_stops=8&lat=48.8566&lon=2.3522&radius_km=25` filters existing QID-linked `visit_sites` by PostGIS `ST_DWithin`. Invalid lat/lon/radius return 400.
- Tour sequence starts at nearest documented site to the chosen origin, then uses nearest-neighbor order. It is **not** a street/walking/driving route.
- Existing `visitability='unknown'` is preserved. No site is created based on distance; no travel time or open-now status is invented.
- A small coordinate form controls destination for this vertical slice. Replace with an accessible place search/geocoding control once a verified place provider and its usage policy are selected.

## Local smoke tests (not executed within GitHub connector)

```bash
git switch cursor/istoria-lead-landmarks-poreia-routes
# Work against a dedicated throwaway PostgreSQL database.
cargo check -p talaria-api
cargo test -p talaria-api --bin talaria
cd web && npm ci && npm run build && npm test
```

Apply only this branch's migration sequence. Run a baseline count of `canonical_events` and `event_evidence`, refresh, then compare counts; they must be unchanged.

For a local Napoleon entity UUID:
```bash
TALARIA_ENABLE_LANDMARK_REFRESH=1 cargo run -p talaria-api -- serve
curl -X POST http://localhost:8080/api/v1/entities/14eb944a-fa31-459a-be7d-1f32488e2bd6/landmarks/refresh
curl 'http://localhost:8080/api/v1/entities/14eb944a-fa31-459a-be7d-1f32488e2bd6/landmarks?limit=12'
curl 'http://localhost:8080/api/v1/entities/14eb944a-fa31-459a-be7d-1f32488e2bd6/visit/itinerary?lat=48.8566&lon=2.3522&radius_km=25&max_stops=6'
```

**Attention**: The sample entity UUID is tied to one prior restored demo snapshot and is **not** guaranteed to exist in another database.

## Required before merge
1. Verify build/test results in a real checkout. This GitHub connector cannot run Cargo or npm in the repository.
2. PostgreSQL integration tests: repeat refresh, verify stable projection, event/evidence counts, lead/non-lead and wrong-person exclusions, revision URLs.
3. Check source lead matching on real Napoleon, Hugo, Curie profiles. Where no original Wikipedia lead is stored, show fallback explicitly.
4. Frontend regression: 390/768/1440 px light/dark and direct deep links. Test list/click/detail without relying on the paginated timeline.
5. Improve tourism usability by replacing coordinate entry with a proper destination search; add routing service only when road network and transport-mode data are verified.
6. Update OpenAPI and add pagination semantics if the landmark catalog grows beyond the bounded editorial shortlist.
7. Keep mutations admin-only with authentication plus rate limiting; environment flag alone does not authorize a user.

Known caveat: exact sentence matching is low-recall and intentionally sacrifices recall for provenance. Introductions with translated wording, paraphrased event titles, or sections not ingested will not match.

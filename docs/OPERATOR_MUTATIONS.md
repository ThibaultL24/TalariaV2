# Operator-only HTTP mutations

These operations have an explicit feature flag **and** require the existing Talaria session / server-side administrator allowlist (`admin_ingest::require_admin`). The public frontend must never treat a feature flag as authorization.

| Endpoint (POST) | Flag |
| --- | --- |
| `/api/v1/entities/{entity_id}/visit/discover` | `TALARIA_ENABLE_VISIT_DISCOVERY` |
| `/api/v1/visit/sites/{site_id}/refresh-osm` | `TALARIA_ENABLE_VISIT_DISCOVERY` |
| `/api/v1/entities/{entity_id}/landmarks/refresh` | `TALARIA_ENABLE_LANDMARK_REFRESH` |

## Deployment / manual verification

1. Leave both flags unset (default): routes return 403 with the existing `*_disabled` codes.
2. Set the flag only in a trusted environment; configure `TALARIA_ADMIN_USER_IDS` as a comma-separated list of **Talaria internal user UUIDs**, not wallet addresses.
3. With flag enabled, an anonymous caller must receive 401; an authenticated non-admin must receive 403; an allowlisted admin may proceed. Check these outcomes on each route.
4. Ensure a fresh SIWE session is used; callers with expired sessions must not be granted access.
5. Verify `offline_only` still prevents live Wikidata / OSM requests after authorization.
6. Validate that legitimate internal/background operators are not relying on unauthenticated requests before promoting the change to production.

This change only reuses existing authorization. Rate limiting, CSRF defenses for cookie-authenticated writes, request budgets and audit logs require a separate production-readiness review. Do not enable the operations publicly until that review.

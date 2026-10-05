# Scholar & Visit modes — Implementation Plan

> **For agent:** REQUIRED SUB-SKILL: use superpowers:executing-plans (or subagent-driven-development) task-by-task after spec approval.

**Goal:** Ship Phase 1 — Scholar/Visit toggle, visit heritage API + map/list — without polluting scholar timeline.

**Architecture:** New `person_visit_heritage` query + Axum routes under `entities/:id/visit/*`; React `lens` URL param + `features/visit/*` UI. Phase 2 adds `visit_opportunities` table + enrich CLI (separate tasks).

**Tech stack:** Rust `talaria-api` / `talaria-store`, SQLx migration, React entity page, MapLibre reuse.

**Spec:** `docs/superpowers/specs/2026-10-04-scholar-visit-modes-design.md`

---

## File map (Phase 1)

| File | Responsibility |
|------|----------------|
| `migrations/0xx_visit_eligible.sql` | `visit_eligible` column + backfill for commemorative types |
| `crates/talaria-store/src/visit.rs` | Heritage list + GeoJSON queries |
| `crates/talaria-api/src/routes/visit.rs` | HTTP handlers |
| `crates/talaria-api/src/routes/mod.rs` | Wire routes |
| `web/src/features/visit/visit-lens-toggle.tsx` | Scholar \| Visit control |
| `web/src/features/visit/entity-visit-map.tsx` | Heritage map |
| `web/src/features/visit/visit-heritage-list.tsx` | List panel |
| `web/src/lib/entity-views.ts` | `getEntityVisitHeritage` client |
| `web/src/features/entity/entity-page.tsx` | Branch tabs by `lens` |
| `web/src/lib/i18n.ts` | Visit strings |

---

### Task 1: Schema `visit_eligible`

**Files:** `migrations/0xx_visit_eligible.sql`

1. Add `visit_eligible BOOLEAN NOT NULL DEFAULT false` on `canonical_events`.
2. Backfill: `true` where `event_type IN ('museum','memorial','statue','street_naming') AND map_eligible`.
3. Set `timeline_eligible = false` on backfill rows if any were true (data repair).
4. `cargo build -p talaria-api` to embed migration.

**Test:** SQL fixture or rust integration test counting heritage rows for test entity.

---

### Task 2: Store layer heritage query

**Files:** `crates/talaria-store/src/visit.rs`, `lib.rs` export

1. `list_visit_heritage(entity_id, limit, cursor)` — filter `visit_eligible AND is_active AND pipeline='person'`.
2. `visit_heritage_geojson(entity_id, bbox optional)` — map_eligible + geom.
3. Reuse `event_to_json_list` shape from API for JSON parity.

**Test:** `cargo test -p talaria-store` with DB fixture or unit test on SQL builder if pattern exists.

---

### Task 3: API routes

**Files:** `crates/talaria-api/src/routes/visit.rs`, router registration

1. `GET /api/v1/entities/:id/visit/heritage` — JSON list + pagination.
2. `GET /api/v1/entities/:id/visit/heritage/geojson` — FeatureCollection (or query param `format=geojson` on single route).
3. `GET /api/v1/entities/:id/visit/anchors` — points for future radius (scholar map points + heritage geom).
4. Document 404 `entity_not_found`; scholar timeline unchanged.

**Test:** `cargo test -p talaria-api` route test with fixture DB or request unit test.

---

### Task 4: Ingest persistence flag (minimal)

**Files:** `crates/talaria-api/src/person_ingest/persist.rs` (or quality gates)

1. When materializing commemorative types, set `visit_eligible=true`, `timeline_eligible=false`.
2. Ensure gates do not mark them `timeline_eligible`.

**Test:** `cargo test -p talaria-quality` if gate tests cover posthumous types.

---

### Task 5: Frontend lens + toggle

**Files:** `visit-lens-toggle.tsx`, `entity-page.tsx`

1. Parse `lens` from `useSearchParams`, default `scholar`.
2. Toggle updates URL without losing `event` param.
3. Scholar tabs unchanged; Visit shows `map | heritage` (stub `now` “coming soon” until Phase 2).

**Test:** `entity-page.test.tsx` — visit lens renders visit map mock.

---

### Task 6: Visit map + heritage list

**Files:** `entity-visit-map.tsx`, `visit-heritage-list.tsx`, `entity-views.ts`

1. Fetch heritage GeoJSON on viewport change (reuse EntityMap bbox pattern).
2. Distinct layer color from scholar events (taxonomy legacy family).
3. List tab: cards with external links from evidence/source when present.

**Test:** manual + `npm run build`.

---

### Task 7: Regression guards

1. Extend `entity-page.test.tsx` or API test: scholar `timeline` response has no `event_type=museum`.
2. README or AGENTS.md one paragraph on Scholar vs Visit.

**Commit:** `feat(explorer): scholar and visit lenses with heritage POIs (phase 1)`

---

## Phase 2 preview (not in this plan)

- Migration `visit_opportunities`
- `talaria visit-enrich --entity … --live`
- `GET …/visit/now` with `ST_DWithin` anchors (geo **a**)
- Visit tab **En ce moment** + map layer

---

## Verification checklist

- [ ] `cargo test -p talaria-quality -p talaria-api` (targeted)
- [ ] `cd web && npm run build && npm test`
- [ ] Manual: Napoleon Scholar timeline unchanged; Visit shows heritage pins if DB has rows

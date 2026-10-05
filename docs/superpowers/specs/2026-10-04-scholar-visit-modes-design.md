# Scholar & Visit — dual experience on one entity

Date: 2026-10-04  
Status: **draft — awaiting review**  
Decisions: approach **C** (separate data projections + shared entity UI); UX entry **A** (Scholar / Visit toggle on entity); live layer geography **(a)** — “En ce moment” defaults to a radius around the subject’s already-geocoded places; user chose full vision **(3)** (heritage + live).

## Goal

Offer two complementary readings of the same historical figure on Talaria:

| Mode | Audience | Promise |
|------|----------|---------|
| **Scholar** (default) | Education, research, historiography | Sourced **life** — timeline, map of occurrences, bibliography, Agora debates. |
| **Visit** | Tourism, cultural travel | **Where to go** — memory sites (museums, memorials, statues…) plus **what’s on now** (exhibitions, events) near the subject’s geography. |

Same `entity_id` / person; different lenses, APIs, and map layers. Never merge commemorative POIs into the biographical timeline.

## Non-goals (phase 1)

- Integrated ticketing or bookings.
- Replacing Agora with travel reviews.
- Inventing dates, venues, or exhibitions without source rows.
- Subject-specific hardcoded rules (Napoleon fixtures are **data**, not gate exceptions).
- A separate “Trips” product or mobile app fork (URL toggle only).

## Product rules (blocking)

1. **Explorer / Scholar** continues to use `pipeline=person` and `person_timeline_projection`. API must **not** return `museum`, `memorial`, `statue`, `street_naming` on default timeline/map (current `NOT IN` filter stays).
2. **Visit — heritage** surfaces those commemorative types (and future `visit_eligible` rows) as **visit POIs**, not life events. Copy/UI: “Lieu de mémoire”, “Inauguré …”, not “Napoleon lived here”.
3. **Visit — live** rows live outside biographical occurrence identity. They expire; scholar canonical rows do not.
4. **Provenance** on every visit card: `source_kind`, `canonical_url`, `fetched_at` (live), evidence link where applicable (heritage).
5. **Opinions** (good museum? overrated?) stay in **claims** / Agora, not in visit facts.

## UX

### Global toggle

On entity routes, persistent control: **Scholar** | **Visit** (stored in URL `?lens=scholar|visit`, default `scholar`; optional `localStorage` mirror).

- Scholar: existing tabs `overview | timeline | map | sources`.
- Visit: tabs `map | heritage | now` (labels i18n FR/EN).

Navbar subtitle may reflect mode (“Life geography” vs “Places to visit”).

### Visit — Map

- Base map (MapLibre) with two toggleable layers: **Heritage** (static POIs) and **Now** (time-bounded pins).
- Default bbox: fit heritage + scholar places, or last user viewport.
- Click pin → detail sheet (title, dates, external link, “Open in Scholar” if a related life event exists).
- Reuse **Event moment search** scoped to visit items.

### Visit — Heritage (list)

- Sort: distance from user (optional geolocation) or by place name.
- Filter: type (museum, memorial, statue…).

### Visit — Now (list)

- Default window: **today − 7d → today + 90d** (configurable).
- **Geography (decision a):** include opportunities whose venue lies within **R km** of any **geocoded heritage or scholar map point** for this entity (default **R = 50** km, cap **500** results). If the subject has **no** geocoded anchor, show empty state with hint to open Scholar map or run ingest — no worldwide scrape.
- Badge: “Ends 12 Apr 2026”, source logo (Europeana, partner, fixture).

### Scholar mode

Unchanged behavior (immersive timeline/map, moment search on life events).

## Data architecture

### Heritage (phase 1 — reuse canonical)

Source: `canonical_events` where:

- `entity_id` = subject,
- `is_active`,
- `pipeline = 'person'`,
- `event_type IN ('museum','memorial','statue','street_naming', …)` (explicit allowlist),
- optional `visit_eligible` boolean (migration, default true for allowlist types when `map_eligible`).

New read model (view or query module): `person_visit_heritage_projection` — same columns needed for GeoJSON + list cards (id, title, summary, time_json, place_label, geom, event_type, evidence count).

**Ingest:** no change required for MVP if commemorative rows already exist; otherwise extend person extractors / posthumous path to materialize them with `timeline_eligible = false`, `visit_eligible = true`, `map_eligible` when geom resolved.

### Live (phase 2 — new table)

```sql
-- illustrative; exact migration in implementation plan
visit_opportunities (
  id UUID PK,
  entity_id UUID FK entities,
  kind TEXT CHECK (kind IN ('exhibition','event','festival','guided_tour','other')),
  title TEXT NOT NULL,
  summary TEXT,
  venue_label TEXT,
  venue_place_id UUID NULL,
  geom GEOGRAPHY(POINT) NULL,
  starts_at TIMESTAMPTZ,
  ends_at TIMESTAMPTZ,
  canonical_url TEXT,
  source_kind TEXT NOT NULL,
  source_record_id TEXT,
  raw_document_id UUID NULL,
  fetched_at TIMESTAMPTZ NOT NULL,
  UNIQUE (entity_id, source_kind, source_record_id)
);
```

Enrichment job `visit-enrich` (CLI): for each entity with geocoded anchors, call connector(s) (Europeana when `EUROPEANA_API_KEY`, else fixtures), upsert opportunities, soft-delete expired rows.

**Radius filter (a):** SQL or post-filter: `ST_DWithin(opp.geom, anchor.geom, R_meters)` where `anchor` = union of scholar + heritage points for entity.

## API (v1)

| Endpoint | Lens | Response |
|----------|------|----------|
| `GET /api/v1/entities/:id/overview` | scholar | unchanged |
| `GET /api/v1/entities/:id/timeline` | scholar | unchanged |
| `GET /api/v1/entities/:id/events` | scholar | unchanged (bbox map) |
| `GET /api/v1/entities/:id/visit/heritage` | visit | `{ events[], geojson? }` paginated |
| `GET /api/v1/entities/:id/visit/now` | visit | query `from`, `to`, `radius_km`, optional `bbox` |
| `GET /api/v1/entities/:id/visit/anchors` | visit | geocoded points used for radius (debug/UI) |

Invalid `?lens=visit` on scholar-only endpoints → **400** `use_visit_endpoints` (mirror `pipeline_retired` pattern).

## Frontend

- `useLensStore` or URL-only `lens` search param.
- `EntityPage` branches tabs by lens.
- `EntityVisitMap`, `VisitHeritageList`, `VisitNowList` under `web/src/features/visit/`.
- Shared moment search adapter: `searchVisitItems`.

## Phased delivery

### Phase 1 (MVP — first ship)

- Toggle Scholar / Visit + Visit map + heritage list/geojson from DB allowlist.
- Empty states + provenance on cards.
- Tests: API filter never leaks heritage into scholar timeline; visit heritage includes commemorative type fixture.

### Phase 2

- Migration `visit_opportunities`, `visit-enrich` CLI, Europeana/fixture connector.
- Visit tab **En ce moment** + map layer with radius (a).
- i18n FR/EN for visit strings.

### Phase 3 (optional)

- User override city/bbox on “Now” (extends geography to **c**).
- Itinerary export (GPX), partner museum feeds.

## Risks

| Risk | Mitigation |
|------|------------|
| Sparse heritage for lightly ingested subjects | Show ingest hint; demo roster subjects pre-enriched |
| Europeana rate limits / key | Fixtures + `source-status`; degrade gracefully |
| Commemorative mis-attributed to subject | Same `SubjectAttribution` / gates as ingest; `visit_eligible` false when rejected |
| UX confusion between modes | Strong visual differentiation; deep links preserve `lens` |

## Success criteria (phase 1)

- Napoleon (Q517) Visit map shows ≥1 heritage POI if present in DB; Scholar timeline count unchanged when toggling.
- No `museum` titles in scholar timeline API response.
- Visit heritage GeoJSON loads in &lt;2s for 200 points on dev hardware.

## Open questions (post-MVP)

- Default radius **R** tuning (50 vs 25 km).
- Whether heritage uses separate pin style from “now” on shared map (recommended: yes).

## References

- `migrations/032_timeline_projection.sql` — scholar projection.
- `crates/talaria-api/src/routes/entity_views.rs` — commemorative exclusion.
- `web/src/lib/event-taxonomy.ts` — museum / memorial types.
- `AGENTS.md` — facts vs claims, single `person` pipeline.

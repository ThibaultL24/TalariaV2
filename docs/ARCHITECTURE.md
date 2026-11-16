# Talaria Engine Architecture

## Overview

The live Explorer product path is person ingest, not the offline Wikipedia dump chain:

```
resolve person → collect sources → extract candidates → ground → gate
→ persist evidence + canonical projection → HTTP API → Explorer UI
```

The dump chain (`extract-pages` → sentences → COSMOS → judge) writes `pipeline='legacy'` and stays an offline tool. It does not drive search-bar ingest or default timeline/map queries.

Onboarding / product path: see root [README.md](../README.md).

## Current boundaries

Istoria, Poreia, and Agora stay separate. A shared field is not a reason to share a table or a screen.

| Concern | Where it lives | What it does not do |
|---|---|---|
| Canonical fact | `talaria-store` `canonical_events`, `person_events` | Store an opinion or a chapter |
| Logical identity | `logical_events.rs`, migration `053` | Treat a shared title or `occurrence_key` as the same fact |
| Supersession | preflight in `logical_events.rs` | Assign an identity when the chain is ambiguous |
| Reconciliation proposal | `logical_reconciliation.rs` | Write a merge |
| Chapter | `editorial.rs`, `routes/editorial.rs`, migrations `054` and `055` | Change a fact's type, evidence, date, or status |
| Quality gate | `talaria-quality` | Call the historical critic or publish a row |
| Historical critic | `talaria-ai` `critic.rs`, `historical_review.rs` | Insert or update a canonical event |
| Supervisor | `talaria-ai` `supervisor.rs`, `supervisor_exec.rs`, `talaria-store` `supervisor.rs` | Turn a plan into a historical write |
| Documentary source | `talaria-sources` | Create a second map point for the same occurrence |
| Istoria UI | `web/src/features/istoria` | Mix visit listings into the biography |
| Poreia | `routes/poreia_*`, `routes/visit*`, `web/src/features/poreia` | Rewrite Istoria facts |
| Agora | `claims`, `routes/agora_graph.rs`, `web/src/features/agora`, `talaria-intuition` | Store the cultural corpus |

`serve` and `migrate` refuse pending migrations unless `TALARIA_APPLY_MIGRATIONS=1`. On `talaria_engine_development`, inspected read-only on 2026-11-16, versions 001 through 054 are applied, including logical identities (`053`) and editorial groups (`054`). Migration `055` (chapter assertions) is not applied. A restart does not apply it.

Public chapter reads are `GET /api/v1/entities/{id}/groups`, `.../groups/{slug}`, and `.../groups/{slug}/events`. A draft answers as a missing chapter. Publication is not that endpoint.

## Where to add something

- A documentary source: a `SourceConnector` in `crates/talaria-sources/src/connectors`, registered in that crate. New documents reinforce an existing occurrence through evidence.
- A historical quality detector: a gate in `talaria-quality` when the decision is Accept, NeedsReview, or Reject. A read-only finding belongs in `talaria-ai/src/critic.rs`. The critic does not publish.
- An Istoria feature: the read model in `talaria-store` and `routes/entity_views.rs` or `routes/events.rs`, then `web/src/features/istoria`.
- A Poreia feature: the visit or Poreia route, then `web/src/features/poreia`.
- An Agora feature: `claims` and `talaria-intuition`, then `web/src/features/agora`.
- An editorial grouping: `editorial_groups`. A membership points at `logical_event_id`.
- An agentic treatment: a plan in `talaria-ai`. Execution and the journal stay in `supervisor_exec.rs` and `talaria-store/src/supervisor.rs`.

Sections further down that describe one migration, such as migration `029`, record that migration. They are not a second product architecture. Audit reports under `docs/` are traces of a review, not the live contract.

## Core Contracts

### Single Person Pipeline

There is exactly one live pipeline: `pipeline='person'`. The explorer uses this exclusively.

| Surface | Contract |
|---|---|
| `canonical_events.pipeline` | CHECK `(pipeline IN ('legacy', 'person'))`, DEFAULT `'person'` |
| HTTP timeline/geojson | Defaults to `pipeline=person` |
| `?pipeline=quality` or `?pipeline=legacy` | Returns `400 pipeline_retired` |
| Explorer search-bar ingest | Uses `run_person_ingest` only |

### Map Eligibility vs Timeline Eligibility

Events have two eligibility flags that control where they appear:

| Flag | Meaning | Requirements |
|------|---------|--------------|
| `timeline_eligible` | Event appears on timeline | Has typed date (not `Unknown`) |
| `map_eligible` | Event appears on map | Has coordinates + `timeline_eligible` + locus event type |

**Key distinction**: An event can be `timeline_eligible=true` but `map_eligible=false` if:
- Coordinates are missing or unresolved
- The event type is not a map locus (e.g., `publication`, `award`)
- The event was rejected by gates for map display

**Map locus event types** (`event_type_is_map_locus`):
- `birth`, `death`, `residence`, `arrival`, `departure`, `passage`
- `meeting`, `exile`, `battle`, `siege`, `education`, `office`
- `marriage`, `divorce`, `travel`, `imprisonment`, `diplomatic`
- `employment`, `work`, `burial`, `treaty`

### Evidence Model

Evidence links canonical events to their sources:

```
canonical_event → event_evidence → raw_document / document_fragment
```

**Evidence triplet** (P0): `source_locator + quoted_text + fragment_id`
- `source_locator`: JSON with `{kind, uri, title}`
- `quoted_text`: Verbatim text supporting the event
- `fragment_id`: Reference to specific document fragment (NEW in P0)
- `evidence_hash`: Deduplication key (md5 of event_id + doc_id + quoted_text)

**Idempotency**: Re-ingest inserts `event_evidence` with `ON CONFLICT DO NOTHING`. Same source twice → no-op. Counts are derived from evidence, not incremented in place.

### TypedTime Model

Dates use `TypedTime` with `kind` and `precision` as separate dimensions:

| Kind | Precision | Example |
|------|-----------|---------|
| `exact` | `day` | 15 August 1769 |
| `exact` | `month` | March 1805 |
| `exact` | `year` | 1805 |
| `range` | `year` | 1804–1815 |
| `approx` | `year` | c. 1799 |
| `unknown` | - | undated |

**Queryable projection**: `canonical_events.date_precision` column stores the derived precision value for efficient filtering without JSON operators.

**Semantic source of truth**: `time_json` JSONB column. The `start_time` column is a SQL projection for ordering/indexing only — never the semantic date.

### Coordinate Precision

Geographic precision is tracked at two levels:

| Column | Values | Purpose |
|--------|--------|---------|
| `location_precision` | `exact`, `approximate`, `centroid`, `unknown` | How coords were obtained |
| `coord_precision_level` | `point`, `city`, `region`, `country`, `unknown` | Zoom-level filtering |
| `uncertainty_radius_m` | meters | Uncertainty radius for display |

**Coord precision level mapping**:
- `point`: < 1km uncertainty
- `city`: 1km–10km
- `region`: 10km–100km
- `country`: > 100km

## Grounding Chain (P0)

Place resolution follows a two-step grounding chain:

```
mention → place identity → geocode
```

**Step 1: Place Identity Resolution**
- Alias gazetteer (~300 known places with embedded QIDs from `fixtures/gazetteer/historical_places.json`)
- Getty TGN (SPARQL endpoint — identity layer, not coordinates)
- World Historical Gazetteer (REST API — identity layer, requires `WHG_API_TOKEN`)
- Wikidata search

**Step 2: Geocoding**
Coordinates come ONLY from:
1. Wikidata P625 (primary)
2. Offline alias gazetteer
3. Wikipedia page coordinates (for followed pages)

**TGN/WHG are identity layers**: They resolve place names to authoritative identifiers which can then be geocoded via P625. They do NOT provide coordinates directly.

**Anti-invention rule**: Never invent coordinates. If geocoding fails, the event stays `timeline_eligible=true`, `map_eligible=false`.

## Authority Bundle

When resolving a person QID, the system extracts and persists authority identifiers:

| Property | Authority | Usage |
|----------|-----------|-------|
| P268 | BnF (Bibliothèque nationale de France) | French authority |
| P214 | VIAF (Virtual International Authority File) | International consolidation |
| P213 | ISNI (International Standard Name Identifier) | Numeric identifier |
| P269 | IdRef | French higher education authority |
| P4258 | Gallica ARK | BnF digital library identifier |

Stored in `entities.authority_ids` as JSONB:
```json
{
  "bnf": "11887067q",
  "viaf": "12345678",
  "isni": "0000 0001 2103 2683",
  "idref": "026794586"
}
```

## Source Connectors

### Explorer Lane (map/timeline)
Only Wikimedia sources contribute to map points:
- Wikipedia (extracts, page coordinates)
- Wikidata (P625 coordinates, structured statements)

Wikisource and Commons supply proof and media. They do not create an event.

### Institutional Sources (evidence only)
These sources provide evidence but do NOT auto-create map pins:
- HAL, Persée, Gallica, BnF, theses.fr, OpenAlex
- Open Library, Internet Archive
- Europeana (with `EUROPEANA_API_KEY`)

**Contract**: Extra sources reinforce the same occurrence via evidence; they never auto-create a new map point.

## Schema Changes (P0)

### Migration 029

1. **Evidence fragment_id**: `event_evidence.fragment_id` links to specific document fragments
2. **Date precision column**: `canonical_events.date_precision` for queryable filtering
3. **Coord precision level**: `canonical_events.coord_precision_level` for zoom filtering
4. **Authority bundle**: `entities.authority_ids` JSONB column
5. **Place identity QID**: `canonical_events.place_identity_qid` for resolved places

## Gates and Rejection

Events go through quality gates before becoming canonical:

| Gate | Effect |
|------|--------|
| `UnknownTime` | `NeedsReview` (not on map) |
| `OtherPersonAgent` | `Reject` (not the subject's event) |
| `SubjectUsedAsComparison` | `Reject` (the named person is only a comparison) |
| `OutsideLifespan` | `Reject` (impossible date) |
| `DuplicateOccurrence` | Reinforce existing (idempotent) |

Rejected events stay in `event_candidates` — they must **never** appear in `canonical_events`.

## Future Work (Deferred from P0)

- IdRef → VIAF → ISNI cascade connectors
- Gallica ALTO/IIIF/texteBrut integration
- Wikidata geoPrecision extraction
- Commons SDC coordinate extraction (P9149/P1259)
- Europeana edm:Place coordinate extraction

## See Also

- [SOURCE_EXPANSION.md](./SOURCE_EXPANSION.md) — Product-truth notes for institutional/catalog sources (Vague A/B)
- [Progressive UX](PROGRESSIVE_UX.md) — product-truth notes for the progressive explorer ingest UX

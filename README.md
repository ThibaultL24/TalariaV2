# Talaria Engine

Historical intelligence pipeline in **Rust**: resolve a person → collect multi-source documents → extract & gate events → timeline + map API + explorer UI.

Cultural biography facts (places, dates, anecdotes, evidence) stay in Talaria (`canonical_events`). Opinions / debates / theories belong in the Intuition lane (`claims` / Agora) — not on the map.

Product doctrine (FR): [`docs/TALARIA_WHITE_PAPER.md`](./docs/TALARIA_WHITE_PAPER.md) — UI: `/whitepaper`.

> **Live product contract:** explorer search, timeline, and geojson are driven only by `pipeline='person'`. The Wikipedia dump → COSMOS → judge chain is an **offline** tooling path (`pipeline='legacy'`). It does **not** populate the explorer until you ingest via the person pipeline.

For Cloud/agent gotchas, see [`AGENTS.md`](./AGENTS.md). Architecture tour: [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md). Precision backlog / gap analysis: [`docs/SEARCH_MAP_QUALITY_GAPS.md`](./docs/SEARCH_MAP_QUALITY_GAPS.md) (when merged).

## Prerequisites

- Rust **stable** (transitive crates currently need rustc ≥ 1.88; prefer latest stable)
- Docker (PostGIS on host port `5433`)
- Node 18+ for the web explorer
- Optional: Python 3 + [COSMOS](https://github.com/ZhangDataLab/COSMOS) for the offline dump extractor (`sidecar/README.md`)
- Optional: `OPENAI_API_KEY` for LLM prose extraction on the person pipeline (without it, structured sources still work)
- Optional data disk: `TALARIA_DATA_ROOT` (default `/mnt/wiki-dump`)

## Quick start (explorer path)

```bash
cp .env.example .env
# DATABASE_URL must use port 5433 to match docker-compose
docker compose up -d

cargo run -p talaria-api -- data-init
cargo run -p talaria-api -- migrate
cargo run -p talaria-api -- serve
# → http://localhost:8080/health

cd web && npm install && npm run dev
# → http://localhost:5173 (proxies /api → :8080)
```

Build the UI into the API binary's static fallback:

```bash
cd web && npm run build
cargo run -p talaria-api -- serve
# → http://localhost:8080
```

### Ingest a person (what fills the map)

Search-bar ingest in the UI, or:

```bash
curl -X POST http://localhost:8080/api/v1/ingest/explorer \
  -H 'content-type: application/json' \
  -d '{"subject":"Napoleon","qid":"Q517"}'
```

Then:

```bash
curl "http://localhost:8080/api/v1/entities/search?q=Napoleon"
curl "http://localhost:8080/api/v1/timeline?person=Napoleon"
curl "http://localhost:8080/api/v1/events/geojson?person=Napoleon"
```

- Omitted `pipeline` defaults to **`person`**.
- `?pipeline=legacy` or `?pipeline=quality` returns **400** `{ "error": "pipeline_retired", "use": "person" }` — never a silent empty list.
- Dump/fixture density scripts do **not** appear in the explorer until person ingest runs.

### What explorer ingest uses today

| Surface | Sources |
|---|---|
| **Explorer / map / timeline** | Wikidata, Wikipedia extracts, WDQS participation, Wikisource/Commons as fact providers |
| **Agora / opinions** | Institutional catalogs (HAL, Gallica, BnF, Persée, theses.fr, OpenAlex, …) |

Institutional catalogs are **not** wired into explorer map pins in the current product path. Extra sources, when they reinforce a fact, add **evidence** on an existing occurrence — they must **not** auto-create a new map point.

## Precision contract (geo + time)

These rules make search and map points trustworthy. Details: `AGENTS.md`, `docs/ARCHITECTURE.md`, `docs/superpowers/specs/2026-08-28-single-person-pipeline-design.md`.

| Concept | Rule |
|---|---|
| **Candidates vs canonical** | `event_candidates` is quarantine. Only **Accept** materializes a `canonical_event`. NeedsReview / Reject never appear on timeline or map. |
| **Occurrence identity** | One point = one historical occurrence keyed by `occurrence_key` (subject + type + role + typed time + place + primary object…). Extra sources reinforce via evidence; they do **not** invent duplicate map points. |
| **Evidence** | Idempotent inserts on `(event_id, raw_document_id, evidence_hash)`. Prefer a full triplet: source locator + quoted text + `fragment_id` when available. Never `source_count++` on the canonical row. |
| **Typed time** | `time` / `time_json` keeps **kind** (`exact` \| `range` \| `approx` \| `unknown`) and **precision** (`day` \| `month` \| `year`) as separate dimensions. Queryable `date_precision` (when present) is a **SQL projection** of that model — not a second time system. `start_time` is for sort/index only — UIs must render from `time`, never slice `start_time`. |
| **Place grounding → geocode** | Resolve place **identity** first (surface preserved, aliases, optional TGN/WHG or authority ids). **Then** attach coordinates (Wikidata P625, gazetteer, page coords). TGN/WHG identity ≠ a map pin. |
| **Map vs timeline** | Events without resolved coordinates stay `timeline_eligible=true`, `map_eligible=false` until place resolution succeeds. Partial dates stay typed; do not coerce year → 1 Jan in semantics. |
| **Person alignment** | Prefer QID; when stubs remain for IdRef/VIAF/ISNI, still read Wikidata `P268` / `P214` / `P213` / `P269` on the QID. |

Person ingest orchestration: `crates/talaria-api/src/person_ingest/` — resolve (QID before entity write) → collect → extract → ground → type → gate → persist. Two passes: Pass 1 birth/death from Wikidata, Pass 2 everything else gated against that lifespan (role-aware).

Destructive rebuild (operator only):

```bash
talaria admin rebuild-person-pipeline \
  --confirm-destruction \
  --backup-manifest ./rebuild-manifest.json
```

## Multi-source catalog (search density)

Density comes from more documents + multi-extractors, not from lowering gates. Announce maturity via `source-status` — stubs are not integrations.

| Maturity | Sources | Typical lane |
|---|---|---|
| Fetch / parse / extract ready (`--live`) | Wikipedia, Wikidata, HAL, Persée, Gallica, theses.fr, OpenAlex, Open Library, Internet Archive, BnF | Explorer (wiki family) / Agora (catalogs) |
| Ready with key | Europeana (`EUROPEANA_API_KEY`) | Agora |
| Extraction-ready proof/media (`--live`) | Wikisource, Commons | Explorer fact providers |
| Still stubs | VIAF, ISNI, IdRef | Alignment layer (read P* on QID meanwhile) |

Operator / batch CLIs (not the explorer search bar): `ingest-quality`, `resolve-places`, `density-report`, `source-status`, `exploration-report`, `connector-report`. See `AGENTS.md`.

## Offline Wikipedia dump path (not the explorer)

Use this for dump density measurement and legacy tooling only. Results write `pipeline='legacy'` and stay off the explorer map/timeline until person ingest.

1. Download from https://dumps.wikimedia.org/enwiki/latest/ (`*-pages-articles-multistream.xml.bz2` + index).
2. Place under `$TALARIA_DATA_ROOT/dumps/`.
3. `extract-pages` → `split-sentences` → `cosmos-extract` (`--mock` without spaCy) → `judge-candidates`.

Napoleon offline fixture (dump path only):

```bash
./scripts/seed_napoleon_pipeline.sh
# Ballpark: ~250+ Napoleon canonical_events, ~50+ places — still NOT pipeline=person
```

COSMOS setup: [`sidecar/README.md`](./sidecar/README.md).

## HTTP surface (v1)

| Method | Path | Role |
|---|---|---|
| GET | `/health`, `/up` | Liveness |
| GET | `/api/v1/status` | Counts + LLM/catalog flags |
| GET | `/api/v1/entities/search` | Entity / Wikidata suggestions |
| GET | `/api/v1/timeline` | Canonical person timeline |
| GET | `/api/v1/events/geojson` | Map-eligible events |
| GET | `/api/v1/events/{id}` (+ `/evidence`) | Detail |
| POST | `/api/v1/ingest/explorer` | Person-pipeline ingest (UI path) |
| POST | `/api/v1/ingest/agora` | Opinion / debate ingest |
| GET | `/api/v1/ingest/{job_id}` | Job status |

## Crates

| Crate | Role |
|---|---|
| `talaria-core` | Config, shared types |
| `talaria-store` | Postgres / sqlx (migrations embedded at compile time) |
| `talaria-sources` | Source connectors + extractors |
| `talaria-quality` | Gates, TypedTime, occurrence keys |
| `talaria-api` | CLI + Axum HTTP + person ingest |
| `talaria-dump` / `talaria-text` / `talaria-cosmos` / `talaria-judge` | Offline dump chain |
| `talaria-wikidata` | Place geocoding + Wikidata helpers |
| `talaria-intuition` | Opinion / debate export lane |

## Web UI

Explorer: sidebar + map (dark nebula), cyan clusters, period waveform bar. Components ported from the Talaria POC (`MapCanvas`, layers, timeline bar, Carto dark style).

## Roadmap (next)

- P0 precision foundations (evidence fragment, queryable precision projections, place grounding before geocode, authority P* bundle)
- Sharper entity search ranking + QID linking
- Catalog depth on explorer (evidence only — no auto pins) after P0
- Gallica texteBrut → ALTO → ContentSearch/IIIF
- Long-running COSMOS HTTP sidecar
- Help-center pages for search precision, map eligibility, and source maturity

## Docs map

| Doc | Audience |
|---|---|
| This README | Humans onboarding to the live product path |
| [`AGENTS.md`](./AGENTS.md) | Agents & Cloud — non-obvious contracts |
| [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) | Stack, data model, runtime flows |
| `docs/superpowers/specs/` | Design specs (person pipeline, harvest, …) |
| `docs/superpowers/plans/` | Implementation plans |
| [`sidecar/README.md`](./sidecar/README.md) | COSMOS + Intuition sidecars |

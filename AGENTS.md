# AGENTS.md

## Cursor Cloud specific instructions

Talaria Engine is a Rust workspace (Wikipedia dump → sentences → phrase-candidates → canonical events → HTTP API) plus a React/Vite web explorer. See `README.md` for the product overview and the full CLI/pipeline reference; only Cloud-specific, non-obvious notes live here.

### Services (dev)
- Postgres + PostGIS: `docker compose up -d` → host port `5433` (see `docker-compose.yml`). Required by everything.
- API + CLI (`talaria-api`, binary `talaria`): `cargo run -p talaria-api -- serve` → port `8080`. `serve` auto-runs migrations. It also serves `web/dist` as a fallback when that dir exists.
- Web explorer (Vite): `cd web && npm run dev` → port `5173`, proxies `/api` → `http://localhost:8080`, so the API must be running for live data.
- COSMOS Python NLP sidecar is OPTIONAL: real extraction needs `sidecar/cosmos` + spaCy models (`sidecar/README.md`); for dev/CI use `cosmos-extract --mock` (rule-based, no models).

### Data ownership (Talaria vs Intuition)
- **Cultural facts stay in Talaria DB**: biography events, places, dates, anecdotes, evidence, dumps → `canonical_events` (+ map/timeline APIs).
- **Avis / théories / débats only → Intuition lane**: table `claims` (`exportable=true`) is the opinion export surface. Do **not** store cultural corpus in Intuition.
- Migration `005_raw_documents_and_claims.sql` adds `raw_documents` (multi-source provenance) and `claims` (opinion lane).

### Non-obvious gotchas
- Rust toolchain: despite the README saying "1.85+", `Cargo.lock` pulls transitive crates that require rustc ≥1.88 (e.g. `home`, `icu_*`). Use the latest `stable` toolchain (`rustup default stable`). The default toolchain is already set to stable in this environment.
- `.env` is required and gitignored. Copy it from `.env.example` (`cp .env.example .env`). The app hard-errors if `DATABASE_URL` is missing. `DATABASE_URL` must use port `5433` to match docker-compose.
- `TALARIA_DATA_ROOT` defaults to `/mnt/wiki-dump` (not writable here); this env sets it to `/home/ubuntu/wiki-dump`. Extracted page text and dump files live there, not in Postgres.
- Docker daemon may not be running on a fresh VM boot. Start it before Postgres: `sudo service docker start` (or `sudo dockerd &`), then `sudo docker compose up -d`. `docker` currently requires `sudo`.
- No compile-time-checked sqlx macros are used, so `cargo build` does NOT need a live database. Only running the CLI/`serve` needs Postgres.
- **sqlx migrations are embedded at compile time** (`talaria-store`). After adding a file under `migrations/`, rebuild before `talaria migrate` (`cargo build -p talaria-api`) or the new migration will not run.
- Geocoding: the judge has a built-in gazetteer (incl. Napoleon places: Ajaccio, Waterloo, Elba, Saint Helena, Austerlitz, …) that sets coordinates offline. `entities/search` still calls Wikidata live for suggestions.
- Mock extractor is `mock:life_events` (not birth-only): born/died/studied/fought/married/crowned/exiled/lived/visited/… Patterns need person + year + place. Prefer bare years (`1769`) for `parse_time_surface`.

### Single person pipeline (explorer contract — do not restore three lanes)

**There is exactly one live pipeline: `pipeline='person'`.** Do not reintroduce `legacy`, `quality`, or parallel explorer orchestration paths. The retired lanes are not coexisting product surfaces.

| Surface | Contract |
|---|---|
| `canonical_events.pipeline` | CHECK `(pipeline IN ('legacy', 'person'))`, DEFAULT `'person'`. Dump/judge writes `'legacy'`. Explorer ingest writes `'person'`. Never `'quality'`. |
| HTTP timeline / geojson | Default `pipeline=person` when omitted. |
| `?pipeline=quality` or `?pipeline=legacy` | **400** `{ "error": "pipeline_retired", "use": "person" }` — never silent empty. |
| Explorer search-bar ingest | `run_explorer_lane` → **`run_person_ingest` only** (`routes/ingest.rs`). No `run_ingest_quality` on the explorer path. |
| Frontend | `timelineSearchParams` defaults to `'person'`; render dates from `time` (`kind`, `precision`, `surface`, `start`, `end`), never slice `start_time`. |

**Candidates vs canonical (blocking):** `event_candidates` is the quarantine. Only `Accept` gates materialize a `canonical_event`. `NeedsReview` and `Reject` stay in `event_candidates` with status and `rejection_codes` — they must **never** appear in `canonical_events`. Timeline/geojson query active canonical rows only; no filter workaround for rejects.

**Evidence is idempotent (blocking):** Never mutate canonical rows with `source_count++`. Re-ingest inserts `event_evidence` with `ON CONFLICT DO NOTHING` on `(event_id, raw_document_id, evidence_hash)`. Same occurrence → one canonical event, many evidence rows. Same source twice → no-op. Counts are derived from evidence, not incremented in place.

**Typed time (blocking):** `time_json` keeps **kind** (`exact` \| `range` \| `approx` \| `unknown`) and **precision** (`day` \| `month` \| `year`) as separate dimensions. `start_time` is a SQL projection for ordering/index only — never the semantic date. Serialise through shared `TypedTime` / `time_to_json`; do not write ad-hoc `{ "kind": "year", "year": … }` shapes.

**Orchestration:** `crates/talaria-api/src/person_ingest/` — resolve (QID before entity write) → collect (Wikimedia + BFS crawl) → extract (structured rules + LLM prose) → ground → type → gate → persist. Two passes mandatory: Pass 1 establishes birth/death from Wikidata; Pass 2 gates everything else. Domain gates live in `talaria-quality` (`apply_gates`, `SubjectAttribution`, role-aware lifespan, occurrence keys).

**Destructive rebuild (explicit operator action):**
```bash
talaria admin rebuild-person-pipeline \
  --confirm-destruction \
  --backup-manifest ./rebuild-manifest.json
```
Purges canonical scope, merges QID duplicates, verifies invariants. **No `TRUNCATE` in migrations.** Re-ingest is a separate explicit step (or `--reingest-subjects`).

**Offline dump tools (not explorer API):** The Wikipedia dump chain (`extract-pages` → `split-sentences` → `cosmos-extract` → `judge-candidates`) writes `pipeline='legacy'` and does **not** drive the explorer, HTTP ingest, or map/timeline. Fixture CLIs (`quality-napoleon-demo`, `quality-fixture`, `ingest-quality`) assemble into `'person'` — they no longer write a separate `'quality'` lane. Do not wire dump/judge back into `run_explorer_lane` or default API queries.

- Migrations `006`–`027`: structure only; rebuild `talaria-store` after adding migrations.
- CLI (explorer-facing): search-bar ingest via HTTP; operator rebuild above.
- CLI (offline): `quality-napoleon-demo`, `quality-report`, `quality-fixture`, `ingest-quality`, `density-report`, `resolve-places`, `source-status` — fixtures and batch jobs only.
- Unit tests: `cargo test -p talaria-quality`. End-to-end person ingest needs Postgres + optional `OPENAI_API_KEY`.

### Multi-source density
- Crate `talaria-sources`: `SourceConnector`, `SourceRegistry`, `PlanSources`, fixture/Wikidata/Wikipedia plus **live catalog** connectors (HAL, Persée, Gallica, theses.fr, OpenAlex, Open Library, Internet Archive, BnF; Europeana when `EUROPEANA_API_KEY` is set). Wikisource/Commons are extraction_ready with `--live`. Remaining (VIAF/ISNI/IdRef) are still stubs.
- Migration `010`: discovery runs, discovered documents, `quality_claims` (+ supports), place_resolutions, eligibility triad (`historically_valid` / `timeline_eligible` / `map_eligible`).
- Extra sources reinforce the same occurrence via idempotent evidence — they must not create duplicate map points or mutate canonical counters.
- Gates unchanged globally; density comes from more documents + multi-extractors (structured, timeline, dense clause, travel, publication, posthumous).
- Tests: `cargo test -p talaria-sources` (fixtures only, no network).

### Person-first density (Napoleon is a fixture)
- Migration `011`: `exploration_targets`, `place_aliases`, `occurrence_key` columns, density targets on runs.
- CLI (JSON-capable, offline/batch): `ingest-quality --live --seed-list fixtures/seeds/napoleon_wiki_titles.txt --target-timeline-events 500 --target-map-events 500 --max-depth 3 --max-documents 10000`, `resolve-places --subject … --all-unresolved`, `density-report --subject … --show-bottlenecks --show-source-coverage --show-unresolved-places`, `source-status`, `exploration-report`, `connector-report`.
- A **point** = one canonical historical occurrence (dated, typed, sourced). Extra sources reinforce the same occurrence via evidence; they never auto-create a new map point.
- Events are **append-only**; corrections use supersession. Partial dates stay typed (`year`/`month`); do not coerce year → 1 Jan in semantics (projection only in `start_time`).
- Events without coords stay `timeline_eligible=true`, `map_eligible=false` until `ResolvePlaces` / page coordinates / aliases succeed.
- Density targets **pilot exploration only** — never invent, never duplicate, never silently lower gates. If budgets exhaust below 500, report `target_not_reached` with bottlenecks.
- For extremely documented subjects (e.g. Napoleon/Q517), density is a **by-product of sources**, not a product floor. Ingest is person-first: all occupations, classes as search priors, no invented map points.
- **Stubs ≠ integrations**: announce maturity via `source-status` only. Wikipedia, Wikidata, HAL, Persée, Gallica, theses.fr, OpenAlex, Open Library, Internet Archive, and BnF are fetch/parse/extract ready with `--live`. Europeana is ready only with `EUROPEANA_API_KEY`. Wikisource/Commons are extraction_ready with `--live` (proof/media, not Events). VIAF/ISNI remain stubs.
- Occurrence identity uses `occurrence_key` (subject+type+role+time+place+primary object…), not display title.
- Seed lists / gazetteer aliases are data, not Napoleon-hardcoded gate rules.

### Offline dump density measurement (Napoleon fixture)
**Not explorer/person ingest.** `./scripts/seed_napoleon_pipeline.sh` rebuilds a synthetic Wikipedia dump and runs the **offline dump chain** (`extract-pages` → `split-sentences` → `cosmos-extract --mock` → `judge-candidates` → `dump-mine`). It ingests real-style English Wikipedia extracts for Napoleon-related pages, filters years to **1765–1865**, seeds opinion rows in `claims` only, and prints SQL density reports at the end. Results stay on the dump path — they do **not** populate `pipeline='person'` or drive search-bar ingest.

```bash
./scripts/seed_napoleon_pipeline.sh
# Expected ballpark after precision filters: ~250+ Napoleon canonical_events, ~50+ places, 0 modern citation noise
# Density output: SQL reports printed by the script (not timeline/geojson — those are the explorer contract for pipeline=person)
```

Event families include battle, diplomatic, meeting, residence, marriage/divorce, exile, office, travel — cultural facts only.

### Seeding demo data without a real Wikipedia dump
`extract-pages` needs a multistream `.xml.bz2` dump + `-index.txt` (format `offset:page_id:title`, one bz2 stream at offset 0 is fine). After a dump exists under `$TALARIA_DATA_ROOT/dumps/`, run:
`extract-pages --dump <file> --skip-existing` → `split-sentences` → `cosmos-extract --mock` → `judge-candidates`. This is an **offline dump path** — results do not appear in the explorer until ingested through the person pipeline. Re-extracting after expanding mock patterns: omit `--skip-existing` or truncate `phrase_candidates` first.

### Lint / test / build
- Rust: `cargo build`, `cargo test`, `cargo clippy --all-targets --all-features` (passes with pre-existing warnings). `cargo fmt --check` may report pre-existing import-ordering diffs under newer rustfmt.
- Web: `npm run build` (`tsc -b && vite build`; this is also the typecheck). No web lint or test scripts are configured.

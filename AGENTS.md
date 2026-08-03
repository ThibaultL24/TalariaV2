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

### Napoleon density demo
Ingests **real English Wikipedia extracts** (biography + battles + treaties + sentimental pages), runs dense prose extraction (`mock:life_events` with page-title context), keeps years in **1765–1865**, and leaves opinion rows in `claims` only.

```bash
./scripts/seed_napoleon_pipeline.sh
# Expected ballpark after precision filters: ~250+ Napoleon canonical_events, ~50+ places, 0 modern citation noise
curl 'http://localhost:8080/api/v1/timeline?person=Napoleon&limit=500'
curl 'http://localhost:8080/api/v1/events/geojson?person=Napoleon&limit=500'
```

Event families include battle, diplomatic, meeting, residence, marriage/divorce, exile, office, travel — cultural facts only.
### Seeding demo data without a real Wikipedia dump
`extract-pages` needs a multistream `.xml.bz2` dump + `-index.txt` (format `offset:page_id:title`, one bz2 stream at offset 0 is fine). After a dump exists under `$TALARIA_DATA_ROOT/dumps/`, run:
`extract-pages --dump <file> --skip-existing` → `split-sentences` → `cosmos-extract --mock` → `judge-candidates`. Then query `/api/v1/timeline?person=...` and `/api/v1/events/geojson`. Re-extracting after expanding mock patterns: omit `--skip-existing` or truncate `phrase_candidates` first.

### Lint / test / build
- Rust: `cargo build`, `cargo test`, `cargo clippy --all-targets --all-features` (passes with pre-existing warnings). `cargo fmt --check` may report pre-existing import-ordering diffs under newer rustfmt.
- Web: `npm run build` (`tsc -b && vite build`; this is also the typecheck). No web lint or test scripts are configured.

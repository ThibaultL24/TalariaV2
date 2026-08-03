# AGENTS.md

## Cursor Cloud specific instructions

Talaria Engine is a Rust workspace (Wikipedia dump → sentences → phrase-candidates → canonical events → HTTP API) plus a React/Vite web explorer. See `README.md` for the product overview and the full CLI/pipeline reference; only Cloud-specific, non-obvious notes live here.

### Services (dev)
- Postgres + PostGIS: `docker compose up -d` → host port `5433` (see `docker-compose.yml`). Required by everything.
- API + CLI (`talaria-api`, binary `talaria`): `cargo run -p talaria-api -- serve` → port `8080`. `serve` auto-runs migrations. It also serves `web/dist` as a fallback when that dir exists.
- Web explorer (Vite): `cd web && npm run dev` → port `5173`, proxies `/api` → `http://localhost:8080`, so the API must be running for live data.
- COSMOS Python NLP sidecar is OPTIONAL: real extraction needs `sidecar/cosmos` + spaCy models (`sidecar/README.md`); for dev/CI use `cosmos-extract --mock` (rule-based, no models).

### Non-obvious gotchas
- Rust toolchain: despite the README saying "1.85+", `Cargo.lock` pulls transitive crates that require rustc ≥1.88 (e.g. `home`, `icu_*`). Use the latest `stable` toolchain (`rustup default stable`). The default toolchain is already set to stable in this environment.
- `.env` is required and gitignored. Copy it from `.env.example` (`cp .env.example .env`). The app hard-errors if `DATABASE_URL` is missing. `DATABASE_URL` must use port `5433` to match docker-compose.
- `TALARIA_DATA_ROOT` defaults to `/mnt/wiki-dump` (not writable here); this env sets it to `/home/ubuntu/wiki-dump`. Extracted page text and dump files live there, not in Postgres.
- Docker daemon may not be running on a fresh VM boot. Start it before Postgres: `sudo service docker start` (or `sudo dockerd &`), then `sudo docker compose up -d`. `docker` currently requires `sudo`.
- No compile-time-checked sqlx macros are used, so `cargo build` does NOT need a live database. Only running the CLI/`serve` needs Postgres.
- Geocoding: the judge has a built-in gazetteer (London, Paris, Berlin, Cambridge, Oxford, Vienna, Rome, Moscow, etc.) that sets coordinates directly, so those places are map-eligible without the network `geocode-places` (Wikidata) step. `entities/search` does call Wikidata live to enrich suggestions.
- The mock COSMOS extractor only recognizes the pattern `"<Name> was born in <YEAR> in <PLACE>."` — useful for seeding demo birth events without spaCy. `wikitext_to_plain` does NOT strip `'''bold'''` markup, so seed sentences should start with the plain person name.

### Seeding demo data without a real Wikipedia dump
`extract-pages` needs a multistream `.xml.bz2` dump + `-index.txt` (format `offset:page_id:title`, one bz2 stream at offset 0 is fine). After a dump exists under `$TALARIA_DATA_ROOT/dumps/`, run:
`extract-pages --dump <file> --skip-existing` → `split-sentences --skip-existing` → `cosmos-extract --mock --skip-existing` → `judge-candidates`. Then query `/api/v1/timeline?person=...` and `/api/v1/events/geojson`.

### Lint / test / build
- Rust: `cargo build`, `cargo test` (17 unit tests, no DB needed), `cargo clippy --all-targets --all-features` (passes with pre-existing warnings). `cargo fmt --check` reports a pre-existing import-ordering diff under newer rustfmt — not introduced by changes.
- Web: `npm run build` (`tsc -b && vite build`; this is also the typecheck). No web lint or test scripts are configured.

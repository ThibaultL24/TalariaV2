# Talaria Engine

Greenfield historical intelligence pipeline in **Rust**: Wikipedia dump → sentences → phrase-candidates (COSMOS) → canonical events → API.

The previous Rails skeleton in this repo was removed.

## Prerequisites

- Rust 1.85+
- Docker (PostGIS)
- Python 3 + [COSMOS](https://github.com/ZhangDataLab/COSMOS) sidecar (see `sidecar/README.md`)
- External disk optional: set `TALARIA_DATA_ROOT` (e.g. `/mnt/wiki-dump`)

## Quick start

```bash
cp .env.example .env
docker compose up -d
cargo run -p talaria-api -- data-init
cargo run -p talaria-api -- migrate
cargo run -p talaria-api -- serve
curl http://localhost:8080/health
```

## Wikipedia dump workflow

1. Download from https://dumps.wikimedia.org/enwiki/latest/
   - `enwiki-*-pages-articles-multistream.xml.bz2`
   - `enwiki-*-pages-articles-multistream-index.txt`
2. Place files under `$TALARIA_DATA_ROOT/dumps/`
3. Build JSONL index (optional cache):

```bash
cargo run -p talaria-api -- dump-index \
  --index /mnt/wiki-dump/dumps/enwiki-YYYYMMDD-pages-articles-multistream-index.txt
```

4. Extract pages into Postgres + `$TALARIA_DATA_ROOT/pages/`:

```bash
cargo run -p talaria-api -- extract-pages \
  --dump /mnt/wiki-dump/dumps/enwiki-YYYYMMDD-pages-articles-multistream.xml.bz2 \
  --limit 1000 \
  --skip-existing
```

5. Split pages into sentences:

```bash
cargo run -p talaria-api -- split-sentences --skip-existing
```

6. Extract phrase-candidates (COSMOS):

```bash
# Dev / CI without spaCy models:
cargo run -p talaria-api -- cosmos-extract --mock --skip-existing

# With COSMOS installed (see sidecar/README.md):
cargo run -p talaria-api -- cosmos-extract --batch-size 32 --skip-existing
```

7. Judge candidates → canonical events:

```bash
cargo run -p talaria-api -- judge-candidates
curl "http://localhost:8080/api/v1/timeline?person=Alan%20Turing"
curl "http://localhost:8080/api/v1/events/geojson?person=Alan%20Turing"
curl "http://localhost:8080/api/v1/entities/search?q=Alan"
```

Dev mock pipeline (no dump required if pages already extracted):

```bash
./scripts/dev-pipeline.sh 100
# or with dump: DUMP=/mnt/wiki-dump/dumps/enwiki-....xml.bz2 ./scripts/dev-pipeline.sh 1000
```

Dense Napoleon demo (cultural events in Talaria; opinion `claims` lane for Intuition):

```bash
./scripts/seed_napoleon_pipeline.sh
curl "http://localhost:8080/api/v1/timeline?person=Napoleon"
curl "http://localhost:8080/api/v1/events/geojson?person=Napoleon"
```

Cultural biography / places / evidence stay in Postgres (`canonical_events`). Table `claims` is reserved for avis/théories exportable to Intuition — not for map facts.

## Crates

| Crate | Role |
|-------|------|
| `talaria-core` | Config, shared types |
| `talaria-store` | Postgres migrations (sqlx) |
| `talaria-dump` | Multistream index reader |
| `talaria-text` | Wikitext cleanup + sentence splitter |
| `talaria-cosmos` | COSMOS sidecar batch runner |
| `talaria-judge` | Rule-based candidate judge |
| `talaria-wikidata` | Wikidata place geocoding |
| `talaria-api` | CLI + Axum HTTP |

## Roadmap (next)

- Entity search + Wikidata QID linking for persons (partial: `/api/v1/entities/search`, scoped geojson)
- COSMOS production sidecar service (long-running) — batch subprocess works; reload models each batch
- CLI `link-entities` to populate `entities.qid`
- Expand judge verb/ontology coverage beyond birth/death/marriage/move/work

## COSMOS (Life Trajectory)

Phrase candidates use [ZhangDataLab/COSMOS](https://github.com/ZhangDataLab/COSMOS) `preprocessing/tuple_extraction.py` (ICWSM 2025 / [arXiv:2406.00032](https://arxiv.org/abs/2406.00032)).

```bash
# one-time setup — see sidecar/README.md
cd sidecar && git clone https://github.com/ZhangDataLab/COSMOS.git cosmos
# create .venv + spaCy en_core_web_trf …

cargo run -p talaria-api -- cosmos-extract --batch-size 4 --skip-existing
cargo run -p talaria-api -- judge-candidates
cargo run -p talaria-api -- geocode-places
```

`--mock` remains for CI without spaCy models.


## Geocoding (Wikidata)

```bash
cargo run -p talaria-api -- geocode-places
```

Caches coords in `place_geocodes` and updates `canonical_events.geom`.

## Web UI

Explorer inspiré du POC Talaria (`/explorer`) : shell sidebar + carte, thème dark nebula, clusters cyan, barre de période waveform.

```bash
cargo run -p talaria-api -- serve
cd web && npm install && npm run dev
# → http://localhost:5173 (proxy API)
```

Build + serve intégré :

```bash
cd web && npm run build
cargo run -p talaria-api -- serve
# → http://localhost:8080
```

Composants portés du POC : `MapCanvas`, `MapSourceManager`, `MapLayers`, `MapInteractions`, `ExplorerMapTimelineBar`, cartes timeline nebula, style Carto dark analytique.

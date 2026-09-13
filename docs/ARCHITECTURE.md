# TalariaV2 — Architecture Tour

> Visite technique du moteur d'intelligence historique Talaria Engine.

## 1. Stack technique

| Couche | Technologies |
|--------|-------------|
| **Backend (core)** | Rust 1.88+, Tokio async runtime |
| **API HTTP** | Axum (port 8080) |
| **Base de données** | PostgreSQL 16 + PostGIS (port 5433 via Docker) |
| **Frontend** | React 19, TypeScript, Vite 7, Tailwind CSS 4, MapLibre GL |
| **NLP sidecar** | Python 3 + COSMOS (spaCy, benepar) — optionnel |
| **Blockchain (opinions)** | TypeScript sidecar pour Intuition testnet |
| **Package managers** | Cargo (Rust workspace), npm (web + sidecars) |

## 2. Structure des répertoires

```
/workspace
├── Cargo.toml              # Rust workspace (11 crates)
├── crates/                  # Crates Rust
│   ├── talaria-api/         # CLI + serveur HTTP Axum (binaire `talaria`)
│   ├── talaria-core/        # Config (AppConfig), types partagés
│   ├── talaria-store/       # Postgres: migrations, requêtes, pools
│   ├── talaria-dump/        # Lecteur multistream Wikipedia XML.bz2
│   ├── talaria-text/        # Nettoyage wikitext, découpage phrases/sections
│   ├── talaria-cosmos/      # Runner batch vers Python COSMOS sidecar
│   ├── talaria-judge/       # Juge rule-based des phrase_candidates
│   ├── talaria-quality/     # Pipeline qualité: gates, typed time, fingerprints
│   ├── talaria-sources/     # Connecteurs multi-sources (HAL, Gallica, BnF…)
│   ├── talaria-wikidata/    # Géocodage Wikidata, WDQS queries
│   └── talaria-intuition/   # Export opinions vers Intuition testnet
├── migrations/              # 28 fichiers SQL (sqlx, embarqués à la compilation)
├── web/                     # Frontend React/Vite
│   ├── src/
│   │   ├── pages/           # HomePage, ExplorerPage, AgoraPage
│   │   ├── components/      # map/, timeline/, explorer/, filters/, layout/
│   │   ├── stores/          # Zustand (explorer-store, locale-store)
│   │   └── lib/             # api.ts, schemas/, geo utilities
│   └── vite.config.ts       # Proxy /api → localhost:8080
├── sidecar/
│   ├── cosmos_batch.py      # Batch runner pour COSMOS phrase extraction
│   └── intuition/           # TypeScript: writeOnChain.ts pour Intuition
├── scripts/                 # dev-pipeline.sh, seed_napoleon_pipeline.sh…
├── fixtures/                # Données de test (BnF, Europeana, seeds…)
└── docker-compose.yml       # PostGIS container (port 5433)
```

## 3. Modèle de données (tables principales)

```
wiki_pages          → Pages Wikipedia extraites (page_id, title, content_hash)
sentences           → Phrases découpées (wiki_page_id, ordinal, text)
entities            → Personnes/lieux (qid Wikidata, wikipedia_title, canonical_name)
phrase_candidates   → Tuples COSMOS (person_surface, time_surface, place_surface, verb_pivot)
candidate_judgments → Scores/labels du juge rule-based
canonical_events    → Faits validés (entity_id, event_type, time_json, place_label, geom)
event_evidence      → Preuves reliant events aux sources (quoted_text, fragment_id)
event_candidates    → Quarantaine avant validation (status: Accept/Reject/NeedsReview)
raw_documents       → Documents multi-sources (Wikipedia, Wikidata, HAL, Gallica…)
soft_claims         → Opinions/théories pour export Intuition (debate_type, evidence_layer)
entity_profiles     → Profils thématiques (occupations, périodes)
place_geocodes      → Cache géocodage Wikidata (place_label → coords)
```

### Invariants clés

- **Pipeline unique** : `canonical_events.pipeline` = `'person'` (explorer) ou `'legacy'` (dump offline). Jamais `'quality'`.
- **Append-only** : Les événements ne sont pas mutés ; corrections via supersession (`superseded_by`).
- **Evidence idempotente** : `event_evidence` avec `ON CONFLICT DO NOTHING` sur `(event_id, raw_document_id, evidence_hash)`.
- **Typed time** : `time_json` contient `kind` (exact/range/approx/unknown) + `precision` (day/month/year). `start_time` est une projection SQL.

## 4. Flux runtime principaux

### 4.1 Pipeline offline (dump Wikipedia)

```
extract-pages → split-sentences → cosmos-extract → judge-candidates → geocode-places
                                        ↓
                              phrase_candidates
                                        ↓
                              canonical_events (pipeline='legacy')
```

Commandes CLI :
```bash
talaria extract-pages --dump enwiki-*.xml.bz2
talaria split-sentences --skip-existing
talaria cosmos-extract --mock  # ou --batch-size 32 avec COSMOS installé
talaria judge-candidates
talaria geocode-places
```

### 4.2 Pipeline explorer (ingest live)

```
POST /api/v1/ingest/explorer  →  run_person_ingest()
       ↓
   resolve QID (Wikidata)
       ↓
   collect (Wikipedia extracts + WDQS participation events)
       ↓
   extract (structured rules + LLM prose si OPENAI_API_KEY)
       ↓
   ground → type → gate → persist
       ↓
   canonical_events (pipeline='person')
```

Deux passes obligatoires :
1. **Pass 1** : Birth/death depuis Wikidata (établit le contexte temporel)
2. **Pass 2** : Tous les autres événements (avec gates lifespan-aware)

### 4.3 API HTTP (routes principales)

| Route | Description |
|-------|-------------|
| `GET /health` | Health check |
| `GET /api/v1/status` | Compteurs DB, config LLM, catalogs |
| `GET /api/v1/entities/search?q=` | Recherche entités (Wikidata suggestions) |
| `GET /api/v1/entities/{id}` | Profil d'une entité |
| `GET /api/v1/timeline?person=` | Timeline JSON d'événements |
| `GET /api/v1/events/geojson?person=` | GeoJSON pour la carte |
| `GET /api/v1/events/{id}` | Détail d'un événement + evidence |
| `POST /api/v1/ingest/explorer` | Ingest qualité (facts + map) |
| `POST /api/v1/ingest/agora` | Ingest historiographie (débats) |
| `GET /api/v1/periods` | Facettes périodes |
| `GET /api/v1/profiles` | Facettes profils |

### 4.4 Frontend (React)

- **HomePage** (`/`) : Landing page
- **ExplorerPage** (`/explorer`) : Carte MapLibre + timeline + sidebar profil
- **AgoraPage** (`/agora`) : Débats historiographiques

State management : Zustand (`explorer-store.ts`)  
API client : `lib/api.ts` (fetchTimeline, fetchGeoJson, startExplorerIngest…)

## 5. Connecteurs multi-sources

Crate `talaria-sources` — connecteurs implémentés :

| Source | Statut | Notes |
|--------|--------|-------|
| Wikipedia | ✅ Live | Extracts REST API |
| Wikidata | ✅ Live | WDQS SPARQL |
| HAL | ✅ Live | archives-ouvertes.fr |
| Persée | ✅ Live | persee.fr |
| Gallica | ✅ Live | BnF gallica.bnf.fr |
| theses.fr | ✅ Live | Thèses françaises |
| OpenAlex | ✅ Live | Métadonnées académiques |
| Open Library | ✅ Live | Internet Archive books |
| Internet Archive | ✅ Live | Archive.org |
| BnF | ✅ Live | data.bnf.fr |
| Europeana | ✅ avec clé | Nécessite `EUROPEANA_API_KEY` |
| Wikisource | ✅ Live | Proof/media only |
| Commons | ✅ Live | Media assets |
| VIAF/ISNI/IdRef | 🔲 Stub | Non implémentés |

## 6. Exécution locale

### Prérequis
```bash
# 1. Copier l'environnement
cp .env.example .env

# 2. Démarrer Docker (si nécessaire)
sudo service docker start

# 3. Lancer Postgres+PostGIS
sudo docker compose up -d

# 4. Build Rust
cargo build

# 5. Migrations + serveur
cargo run -p talaria-api -- serve  # Port 8080, auto-migrate
```

### Frontend dev
```bash
cd web && npm install && npm run dev  # Port 5173, proxy vers :8080
```

### Pipeline de démo (Napoleon fixture)
```bash
./scripts/seed_napoleon_pipeline.sh
# Crée ~250 canonical_events sur pipeline='legacy'
```

### Variables d'environnement clés

| Variable | Requis | Description |
|----------|--------|-------------|
| `DATABASE_URL` | ✅ | Postgres connection (port 5433) |
| `TALARIA_DATA_ROOT` | Défaut `/mnt/wiki-dump` | Stockage dumps/pages |
| `TALARIA_BIND` | Défaut `0.0.0.0:8080` | Adresse API |
| `OPENAI_API_KEY` | Optionnel | LLM overlay (pas d'invention de points) |
| `EUROPEANA_API_KEY` | Optionnel | Connecteur Europeana |

## 7. Points d'attention / Questions ouvertes

### Gotchas techniques
1. **Rustc ≥1.88** requis (malgré README indiquant 1.85+) — transitive deps.
2. **Migrations embarquées** : Après ajout dans `migrations/`, rebuild `talaria-store` avant `talaria migrate`.
3. **Port 5433** : Docker expose Postgres sur 5433, pas 5432 standard.
4. **COSMOS optionnel** : `--mock` pour dev/CI sans spaCy models.

### Architecture notes
- **Pas de tests web** : `npm run build` est le seul check (TypeScript).
- **Pas de CI configurée** visible dans le repo.
- **LLM overlay** : Jamais d'invention de map points, seulement enrichissement prose.
- **Intuition lane** : Opinions exportables séparées des faits culturels (tables `claims`, `soft_claims`).

### Zones incomplètes
- VIAF/ISNI/IdRef : stubs, pas d'intégration réelle
- COSMOS long-running sidecar : actuellement batch subprocess, pas HTTP service
- CLI `link-entities` : mentionné dans roadmap README, non présent
- Certains index uniques conditionnels créés seulement après `rebuild-person-pipeline`

---

*Document généré lors de l'exploration architecture — Septembre 2026*

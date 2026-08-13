# Talaria V2 — Résumé d’implémentation

Date : 2026-08-06  
Objectif : décrire simplement ce qui est **déjà codé**, pas la vision produit.

---

## En une phrase

Talaria lit des dumps Wikipedia/Wikidata (hors ligne), en tire des événements biographiques sourcés dans Postgres+PostGIS, et les expose via une API + un Explorer web (timeline / carte).

---

## Stack

| Couche | Techno |
|--------|--------|
| API / CLI | Rust (Axum, clap, sqlx) — binaire `talaria` |
| DB | Postgres 16 + PostGIS (`docker compose`, port **5433**) |
| NLP | Sidecar Python COSMOS (optionnel) ; en dev : `--mock` |
| Front | React / Vite / MapLibre (`web/`, port **5173**) |
| Dumps froids | Disque via `TALARIA_DATA_ROOT` (ex. `/mnt/wiki-dump`) |

Deux magasins distincts :

1. **Froid** — fichiers dumps / pages extraites (`dumps/`, `pages/`, `wikidata/`)
2. **Chaud** — Postgres (volume Docker `talaria_pg_data`) : tout ce que l’API sert

---

## Crates

| Crate | Rôle |
|-------|------|
| `talaria-core` | Config (`.env`), types partagés |
| `talaria-store` | Accès SQL + migrations embarquées |
| `talaria-dump` | Index / extract multistream Wikipedia |
| `talaria-text` | Nettoyage wikitext, phrases, sections |
| `talaria-cosmos` | Appel batch sidecar COSMOS |
| `talaria-judge` | Jugement candidats → événements (+ classification soft claims) |
| `talaria-wikidata` | Géocodage / dump JSON Wikidata |
| `talaria-quality` | Pipeline qualité (candidats typés, gates, fingerprints) |
| `talaria-sources` | Connecteurs multi-sources + extracteurs densité |
| `talaria-api` | CLI + HTTP + ingest quality / Lot E |

---

## Pipelines d’ingestion (coexistence)

### A — Legacy (dump → carte)

```text
dump WP → extract-pages → split-sentences
  → cosmos-extract [--mock] → judge-candidates
  → geocode-places → canonical_events (pipeline='legacy')
```

### B — Quality / densité (Lot E)

```text
document_snapshots + fragments → event_candidates
  → gates déterministes → canonical_events (pipeline='quality')
  → resolve-places → map_eligible / timeline_eligible
```

CLI utiles : `quality-napoleon-demo`, `ingest-quality`, `density-report`, `resolve-places`.  
Les lignes `legacy` ne sont **jamais** auto-requalifiées en `quality`.

### C — Dump-first (profils / soft claims)

```text
wikidata-ingest → entity_profiles (+ QID / sitelinks)
claims-extract → soft_claims + soft_claim_evidence
split sections → wiki_sections → dossier offline
```

`TALARIA_OFFLINE_ONLY=true` : le dossier narratif ne tape plus MediaWiki en live.

---

## Modèle de données (essentiel)

| Table / famille | Contenu |
|-----------------|--------|
| `wiki_pages`, `sentences`, `wiki_sections` | Texte WP local |
| `entities`, `entity_profiles`, `periods` | Personnes + facettes Explorer |
| `phrase_candidates` → `canonical_events` + `event_evidence` | Chemin legacy |
| `document_*`, `event_candidates`, `quality_claims` | Chemin quality |
| `claims` | Lane **Intuition** (avis / théories, `exportable`) — pas les faits carte |
| `soft_claims` (+ evidence / relations) | Couche Explorer (faits, anecdotes, débats) |
| `place_geocodes`, `place_aliases`, `place_resolutions` | Lieux / géométrie |
| `raw_documents`, `source_discovery_runs`, `discovered_documents` | Provenance multi-sources |

Migrations actuelles : `001` … `014` (sqlx embarqué → rebuild après ajout).

---

## API HTTP (v1)

- `GET /health`, `/api/v1/status`
- `GET /api/v1/entities/search`, `…/entities/{id}`, `…/entities/{id}/claims`
- `GET /api/v1/periods`, `/api/v1/profiles`
- `GET /api/v1/timeline`, `/api/v1/events/geojson`
- `GET /api/v1/events/{id}`, `…/evidence`

Filtres utiles : personne, type, statut épistémique, `profile_slug`, `period_slug`.

Le front Vite proxy `/api` → `:8080`.

---

## Front Explorer

- Carte MapLibre + timeline
- Filtres profil × période × épistémique
- Dossier narratif (offline si flag)
- Panel claims FE : **pas encore** (API soft claims dispo)

---

## État runtime typique (dev)

```bash
docker compose up -d          # Postgres :5433, volume persistant
cargo run -p talaria-api -- migrate
cargo run -p talaria-api -- serve   # :8080
cd web && npm run dev               # :5173
```

Données démo densifiées : `./scripts/seed_napoleon_pipeline.sh`  
Cible Lot E (pilotage) : ≥500 points `map_eligible` pour Napoleon — exploration, pas invention.

---

## Ce qui n’est pas (encore) « prod sources »

- Connecteurs BnF / Gallica / Europeana / Open Library / IA : **stubs** dans `talaria-sources` (maturité via `source-status`)
- COSMOS réel : optionnel ; mock rule-based pour CI/dev
- UI complète des soft claims / débats
- Déploiement managé (Phase 6 ops)

---
## Règles d’implémentation à retenir

1. Faits culturels → Talaria (`canonical_events` / quality). Opinions → `claims` Intuition.
2. Offline-first pour l’ingest cœur ; HTTP live = enrichissement optionnel.
3. Append-only quality + supersession ; pas de coercion année → 1er janvier.
4. Une occurrence = fingerprint / `occurrence_key` ; sources supplémentaires **renforcent**, ne dupliquent pas les points carte.

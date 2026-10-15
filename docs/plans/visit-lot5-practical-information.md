# Visit Lot 5 — Informations pratiques et tourisme (conception)

**Prérequis** : Lot 4 Mérimée validé (notices locales, discover, idempotence). **Scholar inchangé** : `canonical_events` append-only, commémorations hors timeline vie.

## Principe directeur

Séparer deux couches :

1. **Patrimoine stable** (`heritage_sites`, `merimee_notices`) — identité du lieu, protection MH, relations personne sourcées.
2. **Pratique volatile** (`site_practical_facts` ou extension contrôlée de `practical_json`) — horaires, tarifs, accessibilité, fermetures, avec `verified_at`, `source`, `confidence`, TTL de rafraîchissement.

Règle produit : **un monument historique ≠ ouvert au public**. `visitability` ne passe à `verified_open` que sur source pratique explicite (musée, office de tourisme, opérateur certifié).

## Sources candidates

| Source | Données | Licence / accès | Sync | Maturité |
|--------|---------|-------------------|------|----------|
| **OpenStreetMap** | `opening_hours`, `wheelchair`, `museum`, `tourism=*`, site web | ODbL ; Overpass / planet extracts | Diff / Overpass par bbox + `wikidata=Q` | Bon pour POI géolocalisés ; qualité variable |
| **DATAtourisme** | POI culturels, horaires, accessibilité (modèle sémantique) | [data.gouv.fr](https://www.data.gouv.fr/fr/datasets/data-de-datatourisme-en-open-data/) dumps / SPARQL ; conditions d’usage à confirmer par contrat | Dump périodique + delta | Riche pour la France ; mapping vers nos `heritage_sites` via coordonnées + nom + Wikidata |
| **OpenAgenda** | Événements, expositions, festivals | API clé ; [openagenda.com/dev](https://openagenda.com/dev) | Pull par lieu / rayon / QID | Agenda volatile → table `cultural_activities` existante |
| **Musées / sites officiels** | Horaires, billetterie | Scraping interdit par défaut ; privilégier API / flux partenaires | Par site référencé | Haute confiance si `official_url` match |
| **data.enseignementsup / culture.gouv** | Équipements, parfois horaires | À inventorier (pas de POP Mérimée) | Batch | Complément institutionnel |

Endpoints actuels Visit : `practical_json`, `visitability`, `cultural_activities`, `commemorations` — **déjà prévus** mais vides sans connecteur.

## Modèle de données proposé (migration future `036_visit_practical.sql`)

```text
site_practical_snapshots (
  id UUID PK,
  site_id UUID FK heritage_sites,
  source TEXT NOT NULL,          -- osm | datatourisme | openagenda | manual
  source_record_id TEXT,
  practical_json JSONB NOT NULL, -- horaires, prix, accessibilité, langues
  visitability TEXT CHECK (...), -- unknown | verified_open | closed | inaccessible
  confidence TEXT,               -- high | medium | low
  provenance_json JSONB NOT NULL,
  content_hash TEXT NOT NULL,
  fetched_at TIMESTAMPTZ,
  verified_at TIMESTAMPTZ,
  expires_at TIMESTAMPTZ,
  UNIQUE (site_id, source, content_hash)
)

site_practical_links (
  site_id, snapshot_id, is_current BOOLEAN
)
```

- **Pas de fusion** avec `merimee_notices` : lien optionnel `heritage_sites.id` ↔ notice via réconciliation existante.
- Index : `(site_id)`, `(expires_at) WHERE is_current`, GIST si recherche spatiale côté POI OSM.

## API (extensions)

- `GET .../visit/sites` : inclure `practical_current` (snapshot courant) + `visitability` dérivé **uniquement** si snapshot `confidence >= medium` et non expiré.
- `POST .../visit/sites/{id}/refresh-practical` (opérateur) : déclenche connecteur pour un site.
- `GET .../visit/activities` : déjà listé ; alimenter via OpenAgenda avec `provenance_json` obligatoire.

Scholar : aucune nouvelle route sur `canonical_events`.

## Validations

- Refuser horaires sans source ni `verified_at`.
- Refuser `verified_open` si seule source = mention MH ou Wikipedia.
- OSM : exiger tag `opening_hours` + `check_date` si présent ; sinon `confidence=low`.
- Déduplication : même `site_id` + même `content_hash` → no-op.

## Rafraîchissement

- Job CLI `talaria visit refresh-practical --source osm --bbox …` ou `--site-id`.
- TTL par source (ex. OSM 14j, OpenAgenda 6h, DATAtourisme 30j).
- File d’attente par `heritage_sites` publiés uniquement (pas les 46k notices MH brutes).

## Découpage PR (Lot 5)

1. **PR1 — Référentiel pratique** : migration `036`, types Rust, `site_practical_snapshots`, tests Postgres fixtures, API lecture seule.
2. **PR2 — Connecteur OSM** : Overpass par `place_qid` / coordonnées ; mapping `opening_hours` → JSON normalisé ; tests fixture Overpass JSON.
3. **PR3 — OpenAgenda** : ingest `cultural_activities` + filtres entité/site ; pas de mélange avec MH.
4. **PR4 — UI Visit** : afficher horaires/accessibilité avec badge source + date de vérification ; garder « inconnu » par défaut.

## Tests d’intégration

- Fixture OSM + DATAtourisme minimales.
- Site publié + snapshot expiré → API retourne `visitability=unknown`.
- Deux refresh concurrents → un seul `is_current=true`.
- Scholar : test de non-régression `canonical_events` count inchangé après refresh.

## Risques

- Licences mélangées (ODbL vs données ouvertes) → stocker attribution dans `provenance_json`.
- Faux positifs géographiques OSM → **ne pas** lier par proximité seule ; exiger `wikidata` tag ou distance < 50 m + même label normalisé.
- Confusion MH / musée ouvert → copy UI + règles `visitability` strictes.

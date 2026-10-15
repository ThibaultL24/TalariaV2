# Visit Lot 4 — Validation import Mérimée

Date de référence : 2026-10-14. Source officielle : [data.gouv MH](https://www.data.gouv.fr/datasets/immeubles-proteges-au-titre-des-monuments-historiques-3), ressource CSV `20240213-133323` (86 Mo, `Last-Modified` juillet 2026 sur le miroir statique).

## Pipeline (implémentation)

| Phase | Composant | Comportement |
|--------|-----------|--------------|
| Téléchargement / lecture | `visit_cli::run_import_merimee` | Fichier local ou URL ; journalisation `phase=download` / `phase=read_file`. |
| Parse sec | `merimee_csv::dry_run_csv` | Streaming, validation schéma par **en-têtes** ; rejets typés (`missing_reference`, `missing_title`, `invalid_coordinates`, …). |
| Persistance | `merimee::import_merimee_csv` | Canal parse (blocking) → upsert async ; idempotence `content_hash` ; pas de TRUNCATE. |
| Réconciliation | `merimee::reconcile_notices` | Lie `heritage_site_id` si `place_qid` ou `external_ids.merimee` déjà présent — **ne publie pas** de site. |
| Discover | `merimee_hits_for_entity` | Requête indexée sur `person_name_normalized` + filtre `person_named_in` ; pas de scan HTTP du CSV. |

### Runs `partial`

- Statut `partial` si au moins une ligne rejetée **et** au moins une ligne acceptée en persistance.
- Les runs interrompus (opérateur / OOM) restent `running` ou sont clos manuellement en `partial` avec `error_summary`.
- Reprise : `--resume-after-row N` ou `--resume-last-partial` (lit `max(last_row_number)` pour le même `source_uri`).

### Limites connues

- **Double lecture** du fichier sur import local (dry-run + import) : coût CPU/temps doublé sur ~90 Mo.
- **Upsert ligne à ligne** : import complet ~30–60 min selon Postgres ; pas de batch COPY.
- **`Liens_externes` → Wikidata** : champ souvent vide dans le CSV officiel → `place_wikidata_qid` quasi nul ; réconciliation QID rare.
- **`merimee_entity_matches`** : peuplé seulement si réconciliation explicite ; discover ne dépend pas de cette table.

## Métriques fichier source (dry-run sur cache local)

Commande : `talaria visit import-merimee --file .cache/merimee_official.csv` (affiche `phase=parse_dry_run`).

| Métrique | Valeur observée |
|----------|-----------------|
| Octets fichier | 90 231 843 |
| Lignes fichier (wc -l) | 46 327 (1 en-tête + ~46 326 lignes données) |
| Lignes acceptées au parse | **46 230** |
| Lignes rejetées au parse | **96** (`missing_title`) |
| Coût parse sec | 1 passe streaming (rapportée au début de l’import) |

## Métriques PostgreSQL (à jour après import complet)

Utiliser : `talaria visit merimee-status` (JSON) ou requêtes ci-dessous.

```sql
SELECT count(*) FROM merimee_notices;
SELECT count(*) FROM merimee_notice_persons;
SELECT count(DISTINCT notice_reference) FROM merimee_notice_persons;
SELECT count(*) FROM merimee_notices WHERE heritage_site_id IS NOT NULL;
SELECT count(*) FROM heritage_sites WHERE publication_status = 'published';
SELECT status, rows_seen, rows_upserted, rows_unchanged, rows_rejected, last_row_number
  FROM merimee_import_runs ORDER BY finished_at DESC NULLS LAST LIMIT 3;
```

État **avant** re-import complet documenté (import antérieur incomplet) :

| Métrique | Valeur |
|----------|--------|
| Notices persistées | 36 769 |
| Liens personne structurés | 894 |
| Notices liées `heritage_site` | 0 |
| QID Wikidata extraits | 0 |
| Sites Visit publiés | 0 |
| `merimee_entity_matches` | 0 |

**Objectif post-import** : `merimee_notices` ≈ **46 230** (aligné sur dry-run).

### Exécution opérateur (2026-10-14)

```bash
# Cache local (une fois)
curl -sL -o .cache/merimee_official.csv "$DEFAULT_CSV_URL"

# Import complet (une seule instance — verrou advisory Postgres)
RUST_LOG=talaria_store=info cargo run -p talaria-api -- \
  visit import-merimee --file .cache/merimee_official.csv --skip-dry-run

# Idempotence (2ᵉ passe attendue : upserted=0, unchanged≈46230)
cargo run -p talaria-api -- visit import-merimee --file .cache/merimee_official.csv --skip-dry-run

# Audit JSON
cargo run -p talaria-api -- visit merimee-status
```

**Piège observé** : deux imports concurrents (shell relancé) → contention, progression bloquée ~35k lignes. Le code impose désormais `pg_try_advisory_lock(746574601)` (`merimee_import_already_running`).

**Performance** : ~5 000 lignes / ~12 s en upsert ligne à ligne (~2 min pour 46k si une seule instance). Le dry-run double le temps de lecture sur fichier local (désactivable via `--skip-dry-run`).

## Idempotence (2ᵉ exécution)

Attendu sur le même fichier et même `content_hash` :

- `rows_upserted = 0`, `rows_unchanged = rows_seen` (≈ 46 230),
- `count(merimee_notices)` stable,
- aucun doublon de `reference` (PK).

Vérifier aussi : `count(DISTINCT reference) = count(*) FROM merimee_notices`.

## Audit couverture pilotes (Mérimée local → règles catalog)

Commande : `talaria visit merimee-coverage --qid … --label …`

| Personne | QID | Hits Mérimée | Acceptés catalog | Rejetés |
|----------|-----|--------------|------------------|---------|
| Victor Hugo | Q535 | 3 | 3 | 0 |
| Napoléon | Q517 | 11 | 11 | 0 |
| Léonard de Vinci | Q762 | 0 | 0 | 0 |

Relations : uniquement rôles structurés (`habitant célèbre` → `former_residence`, sinon `honoree`) — **pas** l’historique libre.

### Wikidata live (discover, sans persistance)

Test : `cargo test -p talaria-api live_catalog_counts_are_observed -- --nocapture` (réseau + `DATABASE_URL` pour Mérimée local).

Ordre de grandeur **Wikidata seul** (session précédente) : Hugo ~4 sites, Napoléon ~4, Vinci ~3. Mérimée **ajoute** des candidats français structurés, sans remplacer Wikidata.

### Lieux importants absents ou non liés

| Lieu | Motif |
|------|--------|
| Hauteville House (Guernesey) | Hors périmètre MH France ; absent du CSV. |
| Panthéon, Notre-Dame, etc. | Peuvent exister comme notices MH sans `Personnes_liees` pour la personne → **pas** de lien personne–site Mérimée. |
| Vinci | Pas de notices MH françaises liées par champ personne. |
| Rues « Victor Hugo » | Souvent sans personne dans `Personnes_liees` → rejet catalog `person_not_in_source`. |

Aucun critère de validation n’a été assoupli pour gonfler les chiffres.

## Scholar / Visit

- Aucune écriture `canonical_events` lors de l’import Mérimée.
- Publication Visit reste via `discover` + `persist_catalog` (`publication_status=published`).

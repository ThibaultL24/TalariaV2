# Plan Scholar / Visit

## Lot 1 — Fondations (livré)

- Routes `/scholar`, `/visit`, navigation Scholar | Visit | Agora.
- Migration `migrations/033_visit_heritage.sql`.
- Domaine et persistance `crates/talaria-store/src/visit.rs`.
- API `crates/talaria-api/src/routes/visit.rs`.
- La vue de vie exclut `commemoration`.

## Lot 2 — Catalogue patrimonial

`POST /api/v1/entities/{id}/visit/discover` accepte un lot sourcé ou `{"live":true}`.

Un lieu n'est publié que si son identité est un site (classe Wikidata de bâtiment, musée, mémorial, sépulture, ou notice Mérimée dont le champ personnalité contient la personne). Une ville ou une région est rejetée (`settlement_grain`, `region_grain`), même avec des coordonnées. La proximité géographique n'est pas une relation.

Mérimée ne fournit pas d'horaires ni un statut d'ouverture. La visitabilité reste `unknown`.

### Mérimée local (data.gouv.fr, Lot Visit 4)

- Source : [immeubles protégés MH](https://www.data.gouv.fr/datasets/immeubles-proteges-au-titre-des-monuments-historiques-3) — CSV `;` (ressource 2024-02-13), licence dataset `notspecified` sur data.gouv ; ne pas utiliser l’API POP (`404`).
- Migration `migrations/035_merimee_notices.sql` : `merimee_notices`, `merimee_notice_persons`, `merimee_entity_matches`, `merimee_import_runs`. Les notices ne publient pas automatiquement un `heritage_site`.
- Import streaming : `talaria visit import-merimee --file …` ou `--url` (défaut = URL statique data.gouv). Idempotence par `content_hash`, reprise `--resume-after-row`.
- Discover live : `merimee_hits_for_entity` remplace `PopMerimeeConnector` ; relations uniquement via `Personnes_liees_a_l_edifice` (rôle structuré), pas la description libre.
- Fiches API : champ `merimee` sur les sites publiés (référence, protection, commune, historique) quand une notice est liée.
- Validation opérateur : `docs/plans/visit-lot4-merimee-validation.md` ; `talaria visit merimee-status` (audit JSON), `--resume-last-partial`, `--skip-dry-run`.
- Lot 5 (infos pratiques) : `docs/plans/visit-lot5-practical-information.md` — **non implémenté**.

Index : `idx_heritage_sites_place_qid` (déduplication), `idx_heritage_sites_published_geom` (carte des sites publiés). La liste et le GeoJSON filtrent `publication_status = 'published'` et `identity_grain = 'site'`.

La projection biographique ne crée plus de fiche pour un libellé de ville.

## Lot 3 — Interface Visit (livré)

`web/src/features/visit/visit-page.tsx` : recherche, carte, fiches, statut inconnu, lien Scholar, thème existant, layout mobile.

## Lot 4 — Mémoire et activités (livré au contrat)

`POST .../visit/commemorations` et `.../visit/activities` exigent une provenance et une année. Un hommage futur non vérifié est refusé. `bookable` n’est vrai que pour une activité `scheduled`. Aucun agenda externe n’est branché : la liste reste vide tant qu’une source n’a pas écrit une ligne.

## Lot 5 — Itinéraires (livré sans routage)

`POST /api/v1/visit/itineraries` ordonne des sites persistés. `routing_status=unavailable`, `travel_minutes=null`, `straight_line_km` calculé.

TODO : brancher un moteur (OSRM ou équivalent) derrière une variable d’environnement, puis passer `routing_status` à `computed` seulement si la réponse du moteur est réelle.

## Exemple de fiche

```json
{
  "place_label": "Hauteville House",
  "relation": "former_residence",
  "temporal_class": "lived",
  "visitability": "unknown",
  "practical": {},
  "provenance": { "kind": "canonical_event", "event_id": "…" }
}
```

## Performance

Index GIST sur `heritage_sites.geom`, index entité sur les liens, pagination `limit`/`offset` (max 100, carte bornée à 500). Pas d’appel LLM pour afficher une fiche indexée.

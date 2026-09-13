# Évaluation Napoléon (Q517) — Pipeline Person

**Date:** 2026-09-13  
**Base:** `main` @ 89941cf (P0 merged via PR #18)  
**Mode:** Fixture (ingest offline) — le live Wikidata SPARQL timeout pour Napoléon (504 Gateway Timeout)

---

## 1. Comptages globaux

| Métrique | Valeur |
|----------|--------|
| **canonical_events (person, actifs)** | 188 |
| **timeline_eligible** | 188 (100%) |
| **map_eligible** | 104 (55.3%) |
| **has_geom (coordonnées)** | 104 (55.3%) |
| **Lieux uniques** | 97 |
| **Lieux géocodés** | 82 (84.5%) |

### Event Candidates

| Statut | Nombre |
|--------|--------|
| **assembled** (→ canonical_events) | 212 |
| **needs_review** | 130 |
| **rejected** | 25 |
| **Total** | 367 |

### Quality Claims (faits consolidés)

| Statut | Nombre |
|--------|--------|
| **consolidated** | 149 |
| **conflict** | 105 |
| **Total** | 254 |

---

## 2. Répartition par event_type

| Type d'événement | Total | Timeline | Map |
|------------------|-------|----------|-----|
| battle | 89 | 89 | 66 |
| historical_fact | 23 | 23 | 0 |
| residence | 16 | 16 | 10 |
| arrival | 11 | 11 | 6 |
| diplomatic | 9 | 9 | 4 |
| office | 7 | 7 | 0 |
| siege | 6 | 6 | 6 |
| departure | 4 | 4 | 2 |
| marriage | 4 | 4 | 1 |
| commemoration | 3 | 3 | 0 |
| exile | 3 | 3 | 2 |
| travel | 2 | 2 | 2 |
| education | 2 | 2 | 2 |
| military_campaign | 2 | 2 | 0 |
| publication | 2 | 2 | 0 |
| death | 1 | 1 | 1 |
| birth | 1 | 1 | 1 |
| meeting | 1 | 1 | 0 |
| surrender | 1 | 1 | 0 |
| treaty | 1 | 1 | 1 |

Les **battle** dominent (89 événements), cohérent avec la carrière militaire de Napoléon.

---

## 3. Précision temporelle

### Distribution time_json

| Kind | Precision | Nombre |
|------|-----------|--------|
| exact | year | 169 (89.9%) |
| exact | day | 19 (10.1%) |

**Observations:**
- Majorité des événements à précision **année** (169/188)
- 19 événements avec précision **jour** (batailles et traités principalement)
- Aucun événement avec `kind: unknown` ou `kind: approx`

### Exemples de précision jour

| Événement | Date | Surface |
|-----------|------|---------|
| Battle of Montenotte | 1796-04-12 | "12 april 1796" |
| Battle of Lodi | 1796-05-10 | "10 may 1796" |
| Battle of Arcole | 1796-11-15 | "15 november 1796" |
| Treaty of Tolentino | 1797-02-19 | "19 february 1797" |
| Treaty of Campo Formio | 1797-10-17 | "17 october 1797" |

---

## 4. Précision géographique

### Distribution location_precision

| Précision | Nombre |
|-----------|--------|
| (null/vide) | 96 |
| exact | 56 |
| wikidata_p625 | 31 |
| approximate | 2 |
| centroid | 2 |
| wikipedia_page_coordinates | 1 |

**Observations:**
- 92 événements ont une précision de localisation explicite
- 56 événements = coordonnées exactes du lieu historique
- 31 = coordonnées Wikidata P625 (point représentatif)
- Le champ `coord_precision_level` n'est pas rempli (null pour tous)
- `place_identity_qid` = 0% rempli (bottleneck)

---

## 5. Échantillon de points cartographiques (20 exemples)

### Événements clés géolocalisés

| Titre | Type | Lieu | Coords (lat, lon) | Précision | Année |
|-------|------|------|-------------------|-----------|-------|
| birth @ Ajaccio | birth | Ajaccio | 41.93, 8.74 | exact | 1769 |
| death @ Longwood, Saint Helena | death | Longwood, Saint Helena | -15.97, -5.71 | exact | 1821 |
| education @ Brienne | education | Brienne | 48.39, 4.52 | (null) | 1779 |
| education @ Q273480 | education | Q273480 | 48.85, 2.30 | wikidata_p625 | 1784 |
| battle @ Toulon | battle | Toulon | 43.12, 5.93 | (null) | 1793 |
| battle @ Montenotte | battle | Montenotte | 44.37, 8.30 | exact | 1796 |
| battle @ Lodi | battle | Lodi | 45.31, 9.50 | exact | 1796 |
| battle @ Arcole | battle | Arcole | 45.36, 11.28 | exact | 1796 |
| battle @ Rivoli | battle | Rivoli | 45.57, 10.84 | exact | 1797 |
| arrival @ Milan | arrival | Milan | 45.46, 9.19 | exact | 1800 |
| diplomatic @ Amiens | diplomatic | Amiens | 49.89, 2.30 | exact | 1802 |
| diplomatic @ Tilsit | diplomatic | Tilsit | 55.08, 21.88 | (null) | 1807 |
| exile @ Elba | exile | Elba | 42.78, 10.19 | (null) | 1814 |
| exile @ Saint Helena | exile | Saint Helena | -15.97, -5.71 | (null) | 1815 |
| residence @ Q40104 | residence | Q40104 | 41.93, 8.74 | wikidata_p625 | 1769 |
| marriage @ Paris | marriage | Paris | 48.86, 2.35 | (null) | 1796 |
| treaty @ Rivoli | diplomatic | Rivoli | 45.57, 10.84 | exact | 1797 |
| arrival @ Nice | arrival | Nice | 43.70, 7.27 | wikidata_p625 | 1796 |
| arrival @ Paris | arrival | Paris | 48.86, 2.35 | exact | 1797 |
| siege @ Toulon | siege | Toulon | 43.12, 5.93 | (null) | 1793 |

**Couverture géographique:** Europe occidentale (France, Italie, Allemagne), Méditerranée, Atlantique Sud (Sainte-Hélène).

---

## 6. Faits / Anecdotes à faible confiance

### Candidats needs_review (130 total)

Les candidats en `needs_review` sont principalement bloqués par:

| Code de rejet | Nombre |
|---------------|--------|
| competing_place | 81 |
| singleton_cardinality_violation | 11 |
| event_after_subject_death | 9 |
| implausible_age_for_event_type | 8 |
| invalid_place_kind | 2 |

**Exemples typiques needs_review:**
- `arrival` sans année précise
- Lieux en conflit (plusieurs mentions de lieu dans la même phrase)
- Événements post-mortem (commémorations mal datées)

### Quality Claims en conflit (105)

| Type | Consolidated | Conflict |
|------|--------------|----------|
| battle | 78 | 36 |
| historical_fact | 12 | 27 |
| arrival | 4 | 20 |
| residence | 11 | 8 |
| office | 5 | 5 |

Les **historical_fact** et **arrival** ont des taux de conflit élevés — extractions ambiguës ou multiples interprétations.

---

## 7. Jugement de qualité

### Points forts

1. **Couverture événementielle:** 188 événements canoniques couvrant naissance, éducation, batailles, mariages, exils, mort
2. **Batailles bien géolocalisées:** 66/89 batailles (74%) ont des coordonnées
3. **Précision temporelle cohérente:** 100% des événements ont un `time_json.kind = exact`
4. **Naissance et mort précis:** Ajaccio (1769) et Longwood (1821) correctement géocodés
5. **Diversité des types:** 20 types d'événements distincts

### Points faibles / Bottlenecks

1. **Géocodage incomplet:**
   - 84 événements sans coordonnées (44.7%)
   - `historical_fact` = 100% sans coords (21/21)
   - `office` = 100% sans coords (6/6)

2. **Lieux non résolus:** 
   - Amiens, France, Corsica, Moscow — lieux génériques non géocodés
   - QIDs bruts (Q5373953, Q1131971, Q157491) non résolus en coords

3. **Temps majoritairement année:**
   - 90% à précision année, seulement 10% jour
   - Dates exactes disponibles dans les sources mais non extraites

4. **Evidence sparse:**
   - `evidence_count` moyen = 1.13
   - 14 événements multi-source (7.4%)
   - `source_refs` = vide (0 événements avec refs)
   - Table `event_evidence` = 0 lignes

5. **Authority IDs absents:**
   - `place_identity_qid` = 0% rempli
   - `coord_precision_level` = non rempli

---

## 8. Métriques Evidence

| Métrique | Valeur |
|----------|--------|
| events_with_sources (source_count ≥ 1) | 188 |
| events_multi_source (source_count > 1) | 14 |
| max_source_count | 5 |
| avg_evidence_count | 1.13 |
| event_evidence rows | 0 |
| source_refs non-vides | 0 |

**Note:** L'evidence est comptée via `source_count` / `evidence_count` mais pas matérialisée dans `event_evidence`. Le pipeline fixture ne peuple pas cette table.

---

## 9. Résumé numérique final

```
┌────────────────────────────────────────────────────────────┐
│ NAPOLÉON BONAPARTE (Q517) — Pipeline Person (Fixture)     │
├────────────────────────────────────────────────────────────┤
│ canonical_events (actifs)      │ 188                       │
│ timeline_eligible              │ 188 (100%)                │
│ map_eligible                   │ 104 (55.3%)               │
│ has_geom                       │ 104 (55.3%)               │
├────────────────────────────────────────────────────────────┤
│ event_candidates total         │ 367                       │
│   - assembled                  │ 212 (57.8%)               │
│   - needs_review               │ 130 (35.4%)               │
│   - rejected                   │  25 (6.8%)                │
├────────────────────────────────────────────────────────────┤
│ quality_claims                 │ 254                       │
│   - consolidated               │ 149 (58.7%)               │
│   - conflict                   │ 105 (41.3%)               │
├────────────────────────────────────────────────────────────┤
│ Précision temporelle                                       │
│   - day                        │  19 (10.1%)               │
│   - year                       │ 169 (89.9%)               │
├────────────────────────────────────────────────────────────┤
│ Lieux uniques                  │  97                       │
│ Lieux géocodés                 │  82 (84.5%)               │
├────────────────────────────────────────────────────────────┤
│ Events multi-source            │  14 (7.4%)                │
│ Evidence rows                  │   0                       │
└────────────────────────────────────────────────────────────┘
```

---

## 10. Recommandations

1. **Améliorer le géocodage:**
   - Résoudre les QIDs bruts (Q5373953 → coords)
   - Gazetteer pour lieux génériques (France, Corsica, Moscow)

2. **Extraire les dates précises:**
   - Beaucoup de batailles ont des dates au jour près dans Wikipédia
   - Pipeline actuel capture seulement l'année

3. **Peupler event_evidence:**
   - Tracer quoted_text, fragment_id pour chaque événement
   - Permettre l'audit de provenance

4. **Réduire needs_review:**
   - 81 candidats bloqués par `competing_place`
   - Améliorer la désambiguïsation de lieu

5. **Live Wikidata:**
   - Le SPARQL timeout pour Q517
   - Pagination ou requêtes plus ciblées nécessaires

---

**Mode d'évaluation:** Fixture/offline (données extraites de corpus Wikipedia via mock extractor)  
**Avertissement:** Ces résultats ne reflètent pas une ingest live Wikimedia — utiliser pour benchmark du pipeline, pas comme mesure de couverture encyclopédique.

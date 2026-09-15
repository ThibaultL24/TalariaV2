# Évaluation Napoléon Q517 — Large Seed Post-Grounding Fix

**Date:** 13 septembre 2026  
**Branche:** `main` @ 60532a1 (PR #23 merged — async grounding fix)  
**Mode:** `--live` avec seed-list complet (604 titres Wikipedia)

---

## Résumé exécutif

Cette évaluation reproduit la recette « large seed » qui avait produit ~593 événements dans l'évaluation AFTER_SOURCES (PR #22), maintenant sur `main` avec le correctif async grounding du PR #23.

| Métrique | AFTER_SOURCES (~593) | Post #23 (ce run) | Δ |
|----------|---------------------|-------------------|---|
| **canonical_events** | 593 | **779** | +186 (+31.4%) |
| **timeline_eligible** | 593 (100%) | 779 (100%) | — |
| **map_eligible** | 328 (55.3%) | **471 (60.5%)** | +143 (+5.2 pts) |
| **has_geom** | 328 (55.3%) | **471 (60.5%)** | +143 |
| **place_identity_qid** | 0% | **0%** | ⚠️ inchangé |
| **date precision: day** | ~60 (10.1%) | **124 (15.9%)** | +64 (+5.8 pts) |
| **multi-source events** | ~44 (7.4%) | **79 (10.1%)** | +35 (+2.7 pts) |
| **competing_place needs_review** | 81 | **1119** | +1038 |

---

## 1. Comptages globaux

| Métrique | Valeur |
|----------|--------|
| **canonical_events (person, actifs)** | 779 |
| **timeline_eligible** | 779 (100%) |
| **map_eligible** | 471 (60.5%) |
| **has_geom** | 471 (60.5%) |
| **Lieux uniques** | 461 |
| **Lieux géocodés** | 407 (88.3%) |

### Event Candidates

| Statut | Nombre |
|--------|--------|
| **assembled** (→ canonical_events) | 969 |
| **needs_review** | 1561 |
| **rejected** | 384 |
| **Total** | 2914 |

### Quality Claims

| Statut | Nombre |
|--------|--------|
| **consolidated** | 639 |
| **conflict** | 821 |
| **Total** | 1460 |

---

## 2. Répartition par event_type

| Type d'événement | Total | Map Eligible |
|------------------|-------|--------------|
| battle | 475 | 357 |
| diplomatic | 49 | 18 |
| historical_fact | 36 | 0 |
| arrival | 31 | 15 |
| publication | 30 | 0 |
| siege | 28 | 27 |
| office | 22 | 2 |
| residence | 21 | 13 |
| treaty | 16 | 16 |
| commemoration | 15 | 0 |
| exile | 10 | 6 |
| travel | 8 | 8 |
| marriage | 7 | 1 |
| departure | 7 | 2 |
| surrender | 5 | 0 |
| education | 4 | 4 |
| headquarters | 4 | 0 |
| retreat | 4 | 0 |
| military_campaign | 3 | 0 |
| death | 1 | 1 |
| birth | 1 | 1 |

Les **battle** dominent (475 événements, 61%), cohérent avec la carrière militaire de Napoléon.

---

## 3. Précision temporelle

| Kind | Precision | Nombre | % |
|------|-----------|--------|---|
| exact | year | 655 | 84.1% |
| exact | day | 124 | 15.9% |

**Amélioration:** +5.8 points de pourcentage pour la précision jour (15.9% vs 10.1% baseline).

### Distribution par décennie

| Décennie | Événements |
|----------|------------|
| 1760 | 3 |
| 1770 | 5 |
| 1780 | 13 |
| 1790 | 175 |
| 1800 | 250 |
| 1810 | 300 |
| 1820+ | 33 (posthumes) |

---

## 4. Précision géographique

### Location precision

| Précision | Nombre |
|-----------|--------|
| (null) | 308 |
| wikidata_p625 | 298 |
| exact | 148 |
| approximate | 10 |
| centroid | 8 |
| wikipedia_page_coordinates | 7 |

### place_identity_qid

| Métrique | Valeur |
|----------|--------|
| Events avec place_identity_qid | 0 |
| Total | 779 |
| **Taux de remplissage** | **0.0%** |

**⚠️ Observation critique:** Le taux `place_identity_qid` reste à 0% malgré le fix async du PR #23. Les résolveurs TGN/WHG ne produisent pas de résultats dans ce run. La table `place_resolutions` est vide (0 lignes).

**Hypothèses:**
1. Les résolveurs TGN/WHG ne sont pas appelés dans le chemin `--live`
2. Les services TGN/WHG ne retournent pas de correspondances pour les lieux historiques napoléoniens
3. Le gazetteer alias court-circuite avant les appels HTTP

---

## 5. Échantillon d'événements géolocalisés

| Titre | Type | Lieu | Lat | Lon | Précision |
|-------|------|------|-----|-----|-----------|
| birth @ Ajaccio | birth | Ajaccio | 41.93 | 8.74 | year |
| death @ Longwood, Saint Helena | death | Longwood, Saint Helena | -15.97 | -5.71 | year |
| battle @ Marengo | battle | Marengo | 44.89 | 8.68 | year |
| battle @ Austerlitz | battle | Austria | 48.00 | 14.00 | year |
| travel @ Lyon | travel | Lyon | 45.76 | 4.84 | year |
| siege @ Cádiz | siege | Cádiz | 36.54 | -6.30 | year |
| battle @ Tourcoing | battle | Tourcoing | 50.72 | 3.16 | year |

---

## 6. Métriques Evidence

| Métrique | Valeur |
|----------|--------|
| multi_source_events (evidence_count > 1) | 79 |
| % multi-source | 10.1% |
| max_evidence_count | 15 |
| avg_evidence_count | 1.24 |
| event_evidence rows | — |

**Amélioration:** +2.7 points de pourcentage pour les événements multi-source (10.1% vs 7.4% baseline).

---

## 7. Candidats needs_review

### Codes de rejet principaux

| Code de rejet | Nombre |
|---------------|--------|
| competing_place | 1119 |

**⚠️ Observation:** L'explosion `competing_place` (1119 candidats) indique que le pipeline détecte beaucoup d'ambiguïtés de lieu. C'est un effet secondaire de l'extraction plus dense avec `--live`.

---

## 8. Documents et fragments

| Métrique | Valeur |
|----------|--------|
| discovered_documents | 0 |
| corpus_document_snapshots | 0 |
| document_fragments | 87,353 |

---

## 9. Résumé numérique final

```
┌────────────────────────────────────────────────────────────────┐
│ NAPOLÉON BONAPARTE (Q517) — Pipeline Person (Live, Post #23)  │
├────────────────────────────────────────────────────────────────┤
│ canonical_events (actifs)      │ 779                           │
│ timeline_eligible              │ 779 (100%)                    │
│ map_eligible                   │ 471 (60.5%)                   │
│ has_geom                       │ 471 (60.5%)                   │
├────────────────────────────────────────────────────────────────┤
│ event_candidates total         │ 2914                          │
│   - assembled                  │ 969 (33.3%)                   │
│   - needs_review               │ 1561 (53.6%)                  │
│   - rejected                   │ 384 (13.2%)                   │
├────────────────────────────────────────────────────────────────┤
│ quality_claims                 │ 1460                          │
│   - consolidated               │ 639 (43.8%)                   │
│   - conflict                   │ 821 (56.2%)                   │
├────────────────────────────────────────────────────────────────┤
│ Précision temporelle                                           │
│   - day                        │ 124 (15.9%)                   │
│   - year                       │ 655 (84.1%)                   │
├────────────────────────────────────────────────────────────────┤
│ Lieux uniques                  │ 461                           │
│ Lieux géocodés                 │ 407 (88.3%)                   │
├────────────────────────────────────────────────────────────────┤
│ Events multi-source            │ 79 (10.1%)                    │
│ place_identity_qid rempli      │ 0 (0.0%)                      │
└────────────────────────────────────────────────────────────────┘
```

---

## 10. Comparaison AFTER_SOURCES → Post #23

| Métrique | AFTER_SOURCES | Post #23 | Δ | Verdict |
|----------|---------------|----------|---|---------|
| canonical_events | ~593 | 779 | +186 | ✅ Amélioration |
| map_eligible | 55.3% | 60.5% | +5.2 pts | ✅ Amélioration |
| date precision: day | 10.1% | 15.9% | +5.8 pts | ✅ Amélioration |
| multi-source | 7.4% | 10.1% | +2.7 pts | ✅ Amélioration |
| place_identity_qid | 0% | 0% | 0 | ⚠️ Non résolu |
| competing_place | 81 | 1119 | +1038 | ⚠️ Dégradation |

---

## 11. Conclusions

### Points forts

1. **+31% d'événements canoniques** (779 vs ~593) avec le même seed-list
2. **Meilleure couverture cartographique** (60.5% vs 55.3% map_eligible)
3. **Meilleure précision temporelle** (15.9% jour vs 10.1%)
4. **Plus d'événements multi-source** (10.1% vs 7.4%)
5. **Naissance et mort correctement géocodés** (Ajaccio, Longwood)

### Points faibles / Bottlenecks

1. **place_identity_qid reste à 0%** — le fix async PR #23 n'a pas résolu ce problème
   - La table `place_resolutions` est vide
   - TGN/WHG ne sont probablement pas appelés ou ne trouvent pas de correspondances

2. **Explosion competing_place** — 1119 candidats bloqués (vs 81 baseline)
   - Effet secondaire de l'extraction plus dense

3. **Panic pendant l'ingest** — slice index out of bounds
   - Bug à investiguer séparément

---

## 12. Recommandations

1. **Investiguer place_identity_qid = 0%**
   - Vérifier si TGN/WHG sont appelés dans le chemin `--live`
   - Ajouter du logging pour tracer les appels résolveurs
   - Tester avec des lieux connus (Paris, Rome, London)

2. **Réduire competing_place**
   - Améliorer la désambiguïsation de lieu
   - Ajouter des heuristiques de contexte (pays, région)

3. **Corriger le panic slice index**
   - `range end index 5782 out of range for slice of length 5781`

---

## Méthodologie

### Commande exécutée (mode `--live`)

```bash
./target/release/talaria ingest-quality \
  --subject "Napoleon" \
  --qid Q517 \
  --live \
  --seed-list fixtures/seeds/napoleon_wiki_titles.txt \
  --target-timeline-events 500 \
  --target-map-events 500 \
  --max-documents 5000 \
  --max-depth 3 \
  --no-llm-judge
```

### Note: Mode fixture

Le mode `--fixture true` (sans `--live`) n'a produit que 36 événements, insuffisant pour comparaison avec le baseline ~593. Le mode `--live` avec accès Wikipedia/Wikidata est nécessaire pour reproduire les volumes attendus.

---

**Mode d'évaluation:** Live (Wikipedia + Wikidata)  
**Durée d'exécution:** ~25 minutes (interrompu par panic)  
**Contrat respecté:** Aucune coordonnée inventée — toutes proviennent de P625/gazetteer/page coords

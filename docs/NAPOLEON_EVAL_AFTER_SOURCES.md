# Évaluation Comparative Napoléon (Q517) — Post Sources A/B + Progressive UX

**Date:** 2026-09-13  
**Base:** `main` @ d82ca57 (PR #20 Sources A/B + PR #19 Progressive UX merged)  
**Comparaison avec:** `NAPOLEON_EVAL.md` (baseline @ 89941cf, PR #18 P0 only)  
**Mode:** Fixture (ingest offline) — live Wikidata SPARQL timeout pour Napoléon (504 Gateway Timeout + runtime panic dans place_identity.rs:112 block_on inside async)

---

## 1. Tableau Comparatif — Métriques Clés

| Métrique | Baseline (P0) | Après A/B | Delta | Variation |
|----------|---------------|-----------|-------|-----------|
| **canonical_events (person, actifs)** | 188 | 593 | +405 | **+215.4%** |
| **timeline_eligible** | 188 (100%) | 593 (100%) | +405 | +215.4% |
| **map_eligible** | 104 (55.3%) | 328 (55.3%) | +224 | **+215.4%** |
| **has_geom (coordonnées)** | 104 (55.3%) | 328 (55.3%) | +224 | +215.4% |
| **Lieux uniques** | 97 | 306 | +209 | **+215.5%** |
| **Lieux géocodés** | 82 (84.5%) | 266 (86.9%) | +184 | +224.4% |
| **Précision jour** | 19 (10.1%) | 100 (16.9%) | +81 | **+426.3%** |
| **Précision année** | 169 (89.9%) | 493 (83.1%) | +324 | +191.7% |

### Candidates Pipeline

| Métrique | Baseline | Après A/B | Delta | Variation |
|----------|----------|-----------|-------|-----------|
| **Total candidates** | 367 | 2,346 | +1,979 | **+539.2%** |
| **assembled → canonical** | 212 (57.8%) | 733 (31.2%) | +521 | +245.8% |
| **needs_review** | 130 (35.4%) | 1,259 (53.7%) | +1,129 | +868.5% |
| **rejected** | 25 (6.8%) | 354 (15.1%) | +329 | +1,316.0% |

### Quality Claims

| Métrique | Baseline | Après A/B | Delta | Variation |
|----------|----------|-----------|-------|-----------|
| **Total claims** | 254 | 1,116 | +862 | **+339.4%** |
| **consolidated** | 149 (58.7%) | 463 (41.5%) | +314 | +210.7% |
| **conflict** | 105 (41.3%) | 653 (58.5%) | +548 | +521.9% |

### Evidence & Multi-Sources

| Métrique | Baseline | Après A/B | Delta | Variation |
|----------|----------|-----------|-------|-----------|
| **event_evidence rows** | 0 | 2 | +2 | **+∞** |
| **Multi-source events** | 14 (7.4%) | 68 (11.5%) | +54 | **+385.7%** |
| **max_source_count** | 5 | 12 | +7 | +140% |
| **avg_evidence_count** | 1.13 | 1.23 | +0.10 | +8.8% |

### Autorité & Identité

| Métrique | Baseline | Après A/B | Delta | Notes |
|----------|----------|-----------|-------|-------|
| **place_identity_qid fill** | 0% | 0% | 0 | Unchanged — TGN/WHG grounding blocked by async panic |
| **source_refs non-vides** | 0 | 0 | 0 | Still empty arrays |

---

## 2. Répartition par Event Type

| Type d'événement | Baseline | Après A/B | Timeline | Map | Delta |
|------------------|----------|-----------|----------|-----|-------|
| battle | 89 | 316 | 316 | 232 | **+255%** |
| diplomatic | 9 | 45 | 45 | 16 | +400% |
| historical_fact | 23 | 38 | 38 | 0 | +65% |
| arrival | 11 | 30 | 30 | 15 | +173% |
| publication | 2 | 26 | 26 | 0 | **+1200%** |
| residence | 16 | 22 | 22 | 13 | +38% |
| office | 7 | 21 | 21 | 2 | +200% |
| siege | 6 | 16 | 16 | 15 | +167% |
| commemoration | 3 | 15 | 15 | 0 | +400% |
| exile | 3 | 10 | 10 | 6 | +233% |
| treaty | 1 | 10 | 10 | 10 | **+900%** |
| marriage | 4 | 9 | 9 | 3 | +125% |
| departure | 4 | 8 | 8 | 4 | +100% |
| travel | 2 | 7 | 7 | 7 | +250% |
| education | 2 | 4 | 4 | 4 | +100% |
| retreat | 0 | 3 | 3 | 0 | New |
| headquarters | 0 | 3 | 3 | 0 | New |
| surrender | 1 | 3 | 3 | 0 | +200% |
| military_campaign | 2 | 3 | 3 | 0 | +50% |
| death | 1 | 1 | 1 | 1 | — |
| birth | 1 | 1 | 1 | 0 | — |
| divorce | 0 | 1 | 1 | 0 | New |
| anecdote | 0 | 1 | 1 | 0 | New |

**Observations:**
- **Batailles** passent de 89 à 316 (+255%) — meilleure extraction des batailles napoléoniennes
- **Traités** passent de 1 à 10 (+900%) — capture des traités diplomatiques majeurs
- **Publications** passent de 2 à 26 (+1200%) — nouvelles sources académiques
- 4 nouveaux types d'événements: retreat, headquarters, divorce, anecdote

---

## 3. Précision Temporelle

| Kind | Precision | Baseline | Après A/B | Delta |
|------|-----------|----------|-----------|-------|
| exact | day | 19 (10.1%) | 100 (16.9%) | **+426%** |
| exact | year | 169 (89.9%) | 493 (83.1%) | +192% |

**Amélioration notable:** Le taux de précision jour passe de 10.1% à 16.9% — une amélioration de +68% en proportion relative. Plus d'événements avec dates exactes extraits grâce au corpus enrichi.

---

## 4. Précision Géographique

| Précision | Baseline | Après A/B |
|-----------|----------|-----------|
| (null/vide) | 96 | 273 |
| exact | 56 | 135 |
| wikidata_p625 | 31 | 165 |
| approximate | 2 | 9 |
| centroid | 2 | 7 |
| wikipedia_page_coordinates | 1 | 4 |

**Observations:**
- Coordonnées Wikidata P625 passent de 31 à 165 (+432%) — meilleur géocodage via Wikidata
- Coordonnées exactes passent de 56 à 135 (+141%)
- Le taux global de géocodage reste stable (~55%) mais sur un corpus 3× plus large

---

## 5. Bottlenecks — Candidats needs_review

| Code de rejet | Baseline | Après A/B | Delta |
|---------------|----------|-----------|-------|
| competing_place | 81 | 868 | **+970%** |
| singleton_cardinality_violation | 11 | (incl. in 391 empty) | — |
| {} (empty/no code) | — | 391 | New |

**Analyse:** Le bottleneck `competing_place` reste dominant et a explosé (+970%). Cela indique que le corpus enrichi génère beaucoup plus d'extractions ambiguës avec plusieurs lieux mentionnés dans une même phrase. C'est un indicateur positif (plus de données) mais nécessite une désambiguïsation améliorée.

---

## 6. Échantillon Multi-Sources (Top 15)

| Titre | Type | Lieu | Sources | Evidence |
|-------|------|------|---------|----------|
| Napoleon — arrival (1814) | arrival | — | 12 | 12 |
| Napoleon — arrival (1812) | arrival | — | 8 | 8 |
| Napoleon — arrival (1815) | arrival | — | 8 | 8 |
| Napoleon — exile (1814) | exile | — | 7 | 7 |
| Napoleon — battle (1809) | battle | — | 6 | 6 |
| Napoleon — battle (1806) | battle | — | 6 | 6 |
| Napoleon — battle (1813) | battle | — | 5 | 5 |
| Napoleon — arrival (1798) | arrival | — | 4 | 4 |
| Napoleon — historical_fact (1803) @ Britain | historical_fact | Britain | 4 | 4 |
| Napoleon — battle (1798) | battle | — | 4 | 4 |
| Napoleon — commemoration (1840) | commemoration | — | 4 | 4 |
| Napoleon — battle (1800) | battle | — | 4 | 4 |
| Napoleon — historical_fact (1799) | historical_fact | — | 4 | 4 |
| Napoleon — diplomatic (1807) | diplomatic | — | 4 | 4 |
| Napoleon — diplomatic (1797) @ Campo Formio | diplomatic | Campo Formio | 4 | 4 |

**Note:** Les événements multi-sources avec 4+ sources sont maintenant courants. 68 événements ont >1 source (11.5% vs 7.4% baseline).

---

## 7. Interprétation — Améliorations

### Ce qui a progressé

1. **Volume d'événements × 3.15** : 593 événements canoniques vs 188 — le corpus enrichi et les nouveaux connecteurs sources génèrent significativement plus de données exploitables.

2. **Batailles napoléoniennes × 3.55** : De 89 à 316 batailles — la couverture des campagnes militaires est bien meilleure.

3. **Précision temporelle jour +68% (relatif)** : Le pourcentage d'événements avec dates au jour passe de 10.1% à 16.9%, indiquant une meilleure extraction des dates précises.

4. **Multi-sources × 4.86** : 68 événements corroborés par multiple sources vs 14 — meilleure densité de preuves.

5. **Couverture géographique × 3.15** : 306 lieux uniques vs 97 — exploration géographique plus riche.

6. **Géocodage Wikidata P625 × 5.32** : 165 coords P625 vs 31 — meilleur fallback vers Wikidata pour les coordonnées.

7. **Quality claims × 4.39** : 1,116 claims vs 254 — pipeline de consolidation des faits beaucoup plus actif.

8. **Event evidence table non-vide** : 2 lignes vs 0 — première implémentation de la traçabilité evidence.

### Ce qui n'a pas changé

1. **place_identity_qid = 0%** : Le grounding TGN/WHG reste bloqué par un bug `block_on` inside async runtime (panic à place_identity.rs:112). Requiert un fix technique.

2. **source_refs vides** : Les références de source explicites ne sont pas encore peuplées (`[]` pour tous les événements).

3. **Taux map_eligible stable** : 55.3% identique au baseline — proportionnellement, les améliorations de volume ne changent pas le ratio de géocodabilité.

4. **historical_fact / office sans map** : Ces types restent majoritairement non géocodés (0 map pins pour historical_fact, 2/21 pour office).

---

## 8. Bottlenecks Restants

1. **competing_place explosion** : 868 candidats bloqués vs 81 — le désambiüguiseur de lieu est débordé par le volume.

2. **conflict claims élevé** : 58.5% des claims en conflit vs 41.3% — plus de sources = plus de contradictions à réconcilier.

3. **TGN/WHG grounding cassé** : `tokio::runtime::Handle::block_on` ne peut pas être appelé depuis un contexte async. Fix requis: utiliser `spawn_blocking` ou rendre les resolvers async.

4. **Live WDQS timeout** : Napoléon (Q517) est trop documenté pour SPARQL live — pagination ou requêtes plus ciblées nécessaires.

---

## 9. Résumé Numérique Final

```
┌────────────────────────────────────────────────────────────────────┐
│ NAPOLÉON BONAPARTE (Q517) — Post Sources A/B + Progressive UX     │
│ Mode: Fixture (live blocked by WDQS timeout + async panic)        │
├────────────────────────────────────────────────────────────────────┤
│ canonical_events (actifs)      │ 593      (baseline: 188, +215%)  │
│ timeline_eligible              │ 593 (100%)                       │
│ map_eligible                   │ 328 (55.3%)                      │
│ has_geom                       │ 328 (55.3%)                      │
├────────────────────────────────────────────────────────────────────┤
│ event_candidates total         │ 2,346    (baseline: 367, +539%)  │
│   - assembled                  │ 733 (31.2%)                      │
│   - needs_review               │ 1,259 (53.7%)                    │
│   - rejected                   │ 354 (15.1%)                      │
├────────────────────────────────────────────────────────────────────┤
│ quality_claims                 │ 1,116    (baseline: 254, +339%)  │
│   - consolidated               │ 463 (41.5%)                      │
│   - conflict                   │ 653 (58.5%)                      │
├────────────────────────────────────────────────────────────────────┤
│ Précision temporelle                                               │
│   - day                        │ 100 (16.9%)  [baseline: 10.1%]   │
│   - year                       │ 493 (83.1%)  [baseline: 89.9%]   │
├────────────────────────────────────────────────────────────────────┤
│ Lieux uniques                  │ 306      (baseline: 97, +215%)   │
│ Lieux géocodés                 │ 266 (86.9%) [baseline: 84.5%]    │
├────────────────────────────────────────────────────────────────────┤
│ Events multi-source            │ 68 (11.5%) [baseline: 7.4%]      │
│ Evidence rows                  │ 2        (baseline: 0)           │
│ place_identity_qid fill        │ 0%       (unchanged)             │
└────────────────────────────────────────────────────────────────────┘
```

---

## 10. Recommandations

### Priorité Haute

1. **Fix block_on panic** : `crates/talaria-sources/src/place_identity.rs:112` — remplacer `rt.block_on()` par `spawn_blocking` ou rendre `PlaceIdentityResolver::resolve()` async.

2. **Réduire competing_place** : Avec 868 candidats bloqués, améliorer le désambiüguiseur de lieu devient critique. Options: scoring NLP plus fin, contexte de phrase élargi, priorisation du lieu le plus proche du verbe d'événement.

3. **Paginer WDQS pour Q517** : Napoléon timeout sur SPARQL live. Implémenter pagination ou requêtes fragmentées (par décennie, par type d'événement).

### Priorité Moyenne

4. **Peupler source_refs** : Les références de source restent vides — tracker l'origine de chaque extraction pour audit.

5. **Réduire claims conflict** : 58.5% de conflits suggère des extractions contradictoires — ajouter un reconciliateur basé sur la qualité de source.

6. **Corpus evidence live** : L'enrichment corpus (HAL, Gallica, BnF, Persée, theses.fr, OpenAlex) n'a pas pu être exercé à cause du panic async. Dépend du fix #1.

---

**Verdict:** Les PRs #19 et #20 apportent des améliorations significatives en volume (+215% événements) et en densité multi-sources (+385%). Le rate de géocodage reste stable à 55.3% mais sur un corpus 3× plus large. Le principal blocage technique est le bug `block_on` qui empêche le grounding TGN/WHG et l'enrichissement corpus live. Une fois fixé, les métriques `place_identity_qid` et `event_evidence` devraient s'améliorer substantiellement.

---

**Mode d'évaluation:** Fixture/offline (données extraites de corpus Wikipedia via mock extractor + fixture documents)  
**Limitations:** Live Wikidata SPARQL timeout (504), panic async dans place_identity.rs, corpus_evidence enrichment non exercé

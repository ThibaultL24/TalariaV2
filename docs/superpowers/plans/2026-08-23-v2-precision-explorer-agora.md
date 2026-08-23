# Talaria V2 — précision Explorer / Agora

> Plan d’implémentation. À exécuter plus tard avec le playbook executing-plans, une phase à la fois.
> Ce document n’implémente rien : il fige le diagnostic et la séquence de travail.

**Goal:** Remonter la précision V2 au-dessus du POC (identité humaine, points carte sourcés, anecdotes datées) tout en gardant les dumps multi-bases comme colonne vertébrale. L’Explorer n’affiche que des faits de vie datés. L’Agora n’affiche que théories, travaux, publications, controverses, débats.

**Architecture:** Une personne = un QID Wikidata (`P31=Q5`). Les dumps (Wikipedia XML, Wikidata JSON, JSONL catalogues) alimentent `document_snapshots` + fragments. Les extracteurs + gates qualité assemblent `canonical_events` (`pipeline='quality'`, `is_active`). Les catalogues savants alimentent `corpus_documents` + `soft_claims` (jamais la carte). L’HTTP live n’est qu’un complément, jamais le chemin unique de la recherche Explorer.

**Tech Stack:** `talaria-wikidata`, `talaria-api`, `talaria-store`, `talaria-quality`, `talaria-sources`, `talaria-judge`, `talaria-dump`, `web/`.

## Contraintes

- Ne pas requalifier `pipeline='legacy'` en quality.
- Ne pas inventer de coordonnées (pas de centroïde pays → capitale).
- `map_eligible` exige lat/lon **et** un type lieu de vie (`event_type_is_map_locus`).
- Une occurrence = `occurrence_key` ; une source de plus **renforce**, ne duplique pas.
- Classes de personne = priors de recherche, pas veto sur un second métier sourcé.
- Dumps JSONL / Wikipedia extract-pages / Wikidata dump : contrats CLI conservés.
- Intuition / export on-chain : hors scope de ce plan (l’Agora UI lit `soft_claims` + bibliographie).

---

## 0 · Ce que sont « les deux projets »

| Projet | Où | Rôle réel |
|--------|----|-----------|
| **Vision / v1 produit** | `Historical_GPS` (README seul) + POC Rails `codex/bootstrap-talaria-rails-api-application` + backend POC `talaria_ingest` (`Mcp::RankWikipediaPages`, `Talaria::EntitySearchRanker`) | Carte immersive + outil de débats. Promotion **evidence-backed** (candidat → jugement → event). Ranker humain vs statue/œuvre. |
| **Talaria V2** | ce dépôt | Moteur Rust dump + quality + live ingest, Explorer MapLibre, onglet Agora. |

Le POC n’était pas « plus magique ». Il était **plus étroit et plus discipliné** : une identité humaine, des pages wiki classées, une promotion explicite. V2 a plus de sources et plus de chemins, donc plus de façons d’afficher le mauvais sujet, le mauvais point, ou une phrase trop faible / trop stricte.

La démo Napoléon dump (`quality-napoleon-demo`, gazetteer Ajaccio/Waterloo, seeds `fixtures/seeds/napoleon_wiki_titles.txt`) reste le chemin **le plus précis** de V2 — et il n’est **pas** branché sur la recherche Explorer.

```text
Dumps froids (WP / WD / JSONL catalogues)
        │
        ▼
 document_snapshots + fragments
        │
        ├─ extracteurs vie (infobox, structured, timeline, dense, travel, keywords)
        │     → gates → occurrence_key → canonical_events quality
        │     → Explorer : timeline + carte (map_eligible)
        │
        └─ extracteurs savants (historiography, claim, catalogues)
              → corpus_documents + soft_claims
              → Agora : thèses, débats, travaux, controverses
```

---

## 1 · Pourquoi V2 est moins précise (diagnostic sourcé)

### 1.1 Recherche de personnalité

**Symptôme :** taper « Napoléon », « Curie », « Hugo » propose statues, films, taxons, homonymes ; la langue FR est mal classée ; une entité locale bruitée passe devant le vrai humain Wikidata.

**Causes dans le code :**

1. `wbsearchentities` sans `type=item`, sans SPARQL `P31=Q5`, sans sitelinks Wikipedia.
   - Fichier : `crates/talaria-wikidata/src/client.rs` `search_entities`.
2. Score humain = liste d’occupations **anglaises** ; le label est ignoré (`let _ = label`).
   - Fichier : `crates/talaria-wikidata/src/search_rank.rs` `person_search_score`.
   - Une description FR (`empereur des Français`) ne reçoit pas le +50 ; une statue mal pénalisée peut gagner.
3. Langue par défaut `en` ; le front **ne passe pas** `lang`.
   - API : `crates/talaria-api/src/routes/entities.rs` `default_lang`.
   - UI : `web/src/components/search/entity-search-box.tsx` (commit Enter, pas de typeahead).
4. Local-first : `search_local_entities` trie par `COUNT(canonical_events)` **toutes pipelines**, y compris legacy inactif.
   - Fichier : `crates/talaria-store/src/entities.rs`.
   - Si le local est déjà plein de bruit, Wikidata n’est même pas appelé (`items.len() < limit`).
5. Upsert Explorer par **titre** (`upsert_entity_with_kind(..., "person")`) ; le QID est best-effort.
   - Fichier : `crates/talaria-api/src/routes/ingest.rs` `start_lane_ingest`.
   - Deux labels (« Napoléon » / « Napoléon Bonaparte ») = deux entités.

Le POC `EntitySearchRanker` devait booster l’humain et pénaliser statue/œuvre **avant** l’ingest. V2 n’a porté qu’un sous-ensemble anglais.

### 1.2 Points affichables (carte)

**Symptôme :** peu de points, ou beaucoup de mauvais points (batailles d’un écrivain, centroïde d’un pays, publications, anecdotes sans lieu).

**Causes :**

1. Le front plafonne l’Explorer à **80 documents** (`EXPLORER_INGEST_MAX_DOCUMENTS`) alors que Lot E vise 500/500.
   - `web/src/lib/api.ts` vs `DensityTargets` dans `ingest.rs`.
2. GeoJSON **ne filtre pas** `pipeline='quality'` ni `is_active`.
   - `crates/talaria-store/src/canonical_events.rs` `list_geojson_events`.
   - Les compteurs de densité, eux, filtrent quality+active → l’UI et le rapport mentent l’un à l’autre.
3. `map_eligible` à l’assemblage Lot E = coords + `event_type_is_map_locus`. `anecdote` et `publication` sont **hors carte**. `historical_fact` (repli keywords) **est** un locus → bruit géolocalisé.
   - `crates/talaria-quality/src/gates.rs` `event_type_is_map_locus`.
4. `keep_military_typed_event` retourne **toujours `true`**.
   - `crates/talaria-sources/src/person_profile.rs`.
   - Le design 2026-08-19 exigeait P710/P1344 ou une clause d’agent. Conséquence : Hugo hérite des « Battle of » liés depuis See also.
5. Résolution de lieu : gazetteer Napoléon + centroïdes pays (France→Paris) + Wikidata P625. Pays sans ville = fausse précision.
6. Gate **même clause** : année dans la phrase suivante → `NeedsReview`, jamais assemblé. D’où une carte trop vide sur la prose encyclopédique.
7. Explorer live = Wikipedia + Wikidata seulement. Les dumps déjà extraits ne sont pas relus au search.
8. Seeds : Napoléon a `fixtures/seeds/napoleon_wiki_titles.txt`. Tout autre sujet part d’**un** titre (`write_minimal_seed_list`).

### 1.3 Anecdotes et faits historiques

Deux extracteurs se battent :

| Chemin | Fichier | Précision |
|--------|---------|-----------|
| Dump mine legacy | `talaria-judge/src/dump_mine.rs` | Lâche : titre de page + année + lieu + verbe (ou `"occurred"`). Carry année/lieu entre phrases. Seuil juge **0.55**. Épistémique rumor/theory **quand même Accept**. |
| Quality dense | `talaria-quality` analyzer + gates | Stricte : même clause, pas de join, fingerprints, singletons birth/death. Anecdote cues → type `anecdote` **non map**. |

Résultat vécu :

- Sur Napoléon dump : beaucoup de points, dont des faibles « occurred ».
- Sur une personnalité via search live : trop peu de faits (clause unique + 80 docs + lieux non résolus).
- Les vraies anecdotes sourcées n’apparaissent ni en pin (type exclu) ni en Agora (soft claims `debates_only=true` par défaut).

`historically_valid` est forcé à `true` à l’assemble Lot E — ce n’est pas une gate réelle.

### 1.4 Explorer vs Agora (déjà la bonne séparation, mal alimentée)

Le code **a déjà** deux lanes (`LANE_EXPLORER` / `LANE_AGORA`) :

- Explorer : `run_lot_e_density_ingest` → faits datés.
- Agora : `run_corpus_ingest` + `run_historiography_extract` → `corpus_documents` + `soft_claims`. Commentaire explicite : le corpus **ne crée pas** d’events.

Trous produit :

- Select d’une personne ne lance **que** l’Explorer.
- Agora `corpus_limit` défaut **15** notices / provider.
- Taxonomie UI trop courte (`birth_date`, `controversy`…) vs travaux / publications / thèses.
- Catalogues live (HAL, Persée, OpenAlex, Gallica…) n’enrichissent pas les dumps JSONL déjà ingérés.

---

## 2 · Architecture cible (garder les dumps)

Identité d’abord, dumps ensuite, live en dernier.

```text
[Search]
  query + lang
    → SPARQL humans (P31=Q5) + wbsearchentities type=item
    → rerank EntitySearchRanker (FR+EN, sitelinks, occupations P106)
    → 1 QID canonique ; alias labels fusionnés

[Explorer ingest]  (faits de vie uniquement)
  1. Snapshots dumps locaux pour ce QID / sitelinks (WP dump, WD dump, JSONL déjà ingérés)
  2. Si trou : Wikipedia REST + Wikidata statements (même extracteurs)
  3. Extracteurs vie → gates → occurrence_key
  4. resolve-places (P625 du lieu, aliases, page coords ; jamais centroïde pays)
  5. canonical_events quality active → timeline + geojson

[Agora ingest]  (opinions / travaux uniquement)
  1. JSONL dumps catalogues (OpenAlex, HAL, Persée, theses.fr, Gallica, IA, BnF…)
  2. Si trou : connecteurs live --live
  3. historiography-extract → soft_claims
  4. bibliography API ; debates_only=false avec facettes kind
```

Règle d’or : une notice HAL/OpenAlex **n’est jamais** un pin carte. Une bataille n’est un pin que si **cette personne** y participe.

---

## 3 · Phase A — Identité de recherche (fondation)

Fichiers : `crates/talaria-wikidata/src/{client,search_rank}.rs`, `crates/talaria-api/src/routes/entities.rs`, `crates/talaria-store/src/entities.rs`, `web/src/{lib/api.ts,components/search/entity-search-box.tsx,pages/explorer-page.tsx}`.

### A.1 SPARQL humains + type=item

- [ ] `search_entities` : passer `type=item`.
- [ ] Ajouter `search_humans_sparql(query, lang, limit)` : `?item wdt:P31 wd:Q5` + label service, occup. P106, sitelink `{lang}wiki`, description.
- [ ] Fusionner SPARQL + wbsearchentities ; dédup par QID.
- [ ] Test : « Napoleon » → Q517 devant statue/film ; « Curie » → Q7186 devant taxon ; requête FR « Napoléon » idem.

### A.2 Ranker bilingue (vrai port EntitySearchRanker)

- [ ] Scorer label **et** description FR/EN (homme, empereur, physicienne, écrivain…).
- [ ] Pénaliser statue/film/taxon/navire/catégorie **et** équivalents FR.
- [ ] Bonus sitelink Wikipedia + instance human confirmée.
- [ ] Tests unitaires FR et EN dans `search_rank.rs`.

### A.3 API search : QID d’abord, local quality-only

- [ ] `search_local_entities` : `COUNT` seulement `pipeline='quality' AND is_active`.
- [ ] Toujours interroger Wikidata (ne pas skip si le local remplit `limit`).
- [ ] `lang` depuis le navigateur (`browserWikiLang()`), défaut `fr` si `navigator` fr.
- [ ] Réponse : `qid`, `label`, `description` Wikidata (plus le wikipedia_title comme description).
- [ ] Typeahead ≥ 2 caractères (debounce 200 ms) en plus du commit Enter.

### A.4 Une personne = un QID

- [ ] Ingest Explorer : upsert **par QID** ; fusionner les alias labels.
- [ ] Interdire `start_explorer_ingest` sans QID quand online (le search doit en fournir un).
- [ ] Test : « Napoléon » et « Napoléon Bonaparte » → même `entity_id`.

---

## 4 · Phase B — Carte : n’afficher que des occurrences vraies

Fichiers : `canonical_events.rs`, `lot_e.rs`, `person_profile.rs`, `gates.rs`, `extractors/keywords.rs`, `web/src/lib/api.ts`.

### B.1 Contrat GeoJSON / timeline

- [ ] `list_geojson_events` et `list_timeline_events` : `pipeline = 'quality' AND is_active`.
- [ ] GeoJSON : `map_eligible = true AND geom IS NOT NULL AND event_type_is_map_locus`.
- [ ] Timeline : `timeline_eligible` (peut être sans geom).
- [ ] Test SQL / API : une ligne legacy map_eligible n’apparaît plus.

### B.2 Participation militaire

- [ ] Remplacer `keep_military_typed_event` always-true par le contrat du spec 2026-08-19 :
  - garder battle/siege ssi signal personne (P607/P241/P410/P710/P1344) **ou** clause d’agent sur **sa** bio ;
  - drop si le seul lien est un titre `Battle of` / `Bataille de` hors bio.
- [ ] Tests panel : Curie 0 battle ; Hugo 0 battle via See also ; Napoléon garde Austerlitz.

### B.3 Lieux

- [ ] Interdire gazetteer pays→capitale pour `map_eligible` (garder le label pour la timeline).
- [ ] Chaîne : P625 du QID lieu → page coords → alias `place_aliases` → sinon timeline only.
- [ ] `historical_fact` (fallback keywords) : **plus** un map locus. Soit typer vraiment, soit timeline-only.
- [ ] Anecdote **avec** lieu résolu + année + sujet agent : nouveau type `anecdote` **timeline** ; pin seulement si `event_type_is_map_locus` **ou** flag explicite `anecdote_located` (naissance d’un lieu précis, pas une légende flottante).

### B.4 Densité sans invention

- [ ] Front : `EXPLORER_INGEST_MAX_DOCUMENTS` 80 → budget serveur (ex. 400, déjà le cap API).
- [ ] Seeds : à partir du QID, sitelinks + pages liées **datées** dans la fenêtre de vie (déjà `dated_wikilink_titles`) ; ne plus dépendre du fichier Napoléon.
- [ ] Rapport `target_not_reached` + bottlenecks si budget épuisé (déjà Lot E) — l’exposer dans l’UI Explorer.

---

## 5 · Phase C — Dumps comme chemin Explorer (pas seulement CLI)

Fichiers : `dump_ingest.rs`, `dump_events.rs`, `dump_cosmos.rs`, `routes/ingest.rs`, `talaria-dump`.

Le CLI dump existe déjà et doit rester :

```text
dump ingest (JSONL catalogues / extraits)
  → dump cosmos-extract (ou mock)
  → dump extract-events → dump canonicalize
Wikipedia XML : extract-pages → split-sentences → cosmos-extract → judge (legacy, coexistence)
Wikidata dump : occupations / P569 / P570 / sitelinks
```

### C.1 Explorer lit d’abord le chaud local

- [ ] Au select QID : si des `document_snapshots` existent pour ce sujet, **assembler** depuis ces fragments avant tout HTTP.
- [ ] Même extracteurs / gates que Lot E (`default_extractor_stack` + `apply_gates`).
- [ ] Live Wikipedia seulement pour les sitelinks manquants.

### C.2 JSONL multi-bases inchangé

- [ ] `DumpReader` / `JsonlDumpReader` : aucun changement de contrat `DumpRecord`.
- [ ] Router `source_kind` :
  - wiki / wikidata / biographical JSONL → lane Explorer (faits) s’ils portent date+lieu+agent ;
  - openalex / hal / persee / theses / gallica / ia / bnf → lane Agora uniquement.
- [ ] Test : un JSONL OpenAlex n’insère pas de `canonical_events`.

### C.3 Mine d’anecdotes dump : resserrer sans tuer le rappel

- [ ] `dump_mine.rs` : supprimer le verbe par défaut `"occurred"` ; exiger un cue de `VERB_CUES` **ou** `ANECDOTE_CUES`.
- [ ] Carry année/lieu : seulement avec `has_subject_hook` (déjà) **et** fenêtre de vie du QID (plus −4000..2100 global).
- [ ] Juge : `theory` / `rumor` / commemorative → **soft_claim**, pas `canonical_events`.
- [ ] Seuil 0.55 : monter pour `historical_fact` générique ; garder plus bas pour infobox/structured.

---

## 6 · Phase D — Faits vs anecdotes vs débats (épistémique)

Fichiers : `talaria-quality` gates/analyzer, `talaria-judge/src/claims.rs`, `talaria-api` claim_extract / historiography, `web` agora-taxonomy + explorer-page.

### D.1 Trois couches visibles

| Couche | Stockage | UI |
|--------|----------|----|
| Fait de vie daté | `canonical_events` quality | Explorer carte + timeline |
| Anecdote sourcée | `canonical_events` type `anecdote` **ou** `soft_claims` kind=`anecdote` si contestée | Timeline Explorer ; pin ssi lieu précis |
| Théorie / débat / thèse / ouvrage | `soft_claims` + `corpus_documents` | Agora uniquement |

- [ ] `historically_valid` : false si gate NeedsReview / épistémique non `attested`.
- [ ] API claims : `debates_only` défaut **false** sur l’onglet Agora ; facettes `claim_kind`.
- [ ] Taxonomie Agora : `work`, `thesis`, `publication`, `controversy`, `historiography`, `theory`, `debate` (étendre `agora-taxonomy.ts`).

### D.2 Même-clause vs contexte

- [ ] Garder le reject `CrossClauseJoin` pour **assembler un pin**.
- [ ] Autoriser un candidat `NeedsReview` timeline-only si année dans la phrase **adjacente** du même paragraphe **et** même sujet (pas de pin tant que lieu non dans la clause).
- [ ] Tests : « Il naît en 1769. Ajaccio l’accueille. » → birth timeline, pas deux pins inventés.

### D.3 Lancer les deux lanes depuis l’UI

- [ ] Select personne : Explorer auto (déjà) + bouton Agora inchangé.
- [ ] Option « Collecter aussi l’Agora » après le premier lot Explorer (ne pas bloquer la carte sur HAL).
- [ ] Compteurs profil : faits carte / faits timeline / items Agora séparés.

---

## 7 · Phase E — Harness de régression (empêche de reculer)

Panel **sans réseau** (fixtures) — reprendre et étendre `docs/superpowers/specs/2026-08-19-universal-person-ingest-design.md` :

| Sujet | Search | Carte | Agora |
|-------|--------|-------|-------|
| Napoléon Q517 | Q517 #1 FR et EN | birth Ajaccio, Waterloo participation, **pas** statue Vendôme comme vie | controverses / ouvrages |
| Marie Curie | humaine #1 | birth/education/lab ; **0** battle | publications, Nobel comme travail |
| Victor Hugo | humaine #1 | exile/residence/publication timeline ; **0** Waterloo | débats littéraires |
| Christophe Colomb | humaine #1 | voyages ; origines catalane/juive/suisse **Agora only** | theories of origin |
| Cléopâtre | humaine #1, années ≤ 0 | office/residence ; pas clamp 1000–2100 | — |

- [ ] Tests `talaria-wikidata` search.
- [ ] Tests `talaria-sources` participation.
- [ ] Tests API geojson filtre quality.
- [ ] Tests dump JSONL Agora ≠ map.
- [ ] `web` : pas de test runner aujourd’hui ; ajouter au moins des tests vitest sur le mapping search `lang` + filtre geojson client si vous touchez le store.

Commandes de preuve après chaque phase :

```bash
# illustrative only
cargo test -p talaria-wikidata -p talaria-quality -p talaria-sources -p talaria-judge
cargo test -p talaria-api --dump-ingest -- --ignored   # si tests dump
cd web && npm run build
```

---

## 8 · Ordre d’exécution recommandé

1. **A** (search/QID) — sans ça, tout ingest part du mauvais sujet.
2. **B.1 + B.2** (filtre API + participation) — la carte arrête de mentir.
3. **C.1** (dumps → Explorer) — ramène la précision Napoléon-like à toute personnalité déjà dumpée.
4. **C.3 + D** (mine + épistémique + Agora taxonomie).
5. **B.3–B.4 + C.2** (lieux, budgets, router JSONL).
6. **E** en continu (un test du panel à chaque PR).

Ne pas commencer par « mettre 500 documents » : ça densifie le bruit.

---

## 9 · Hors scope (volontaire)

- COSMOS spaCy production (le mock/heuristic reste ; le vrai sidecar est orthogonal).
- Stubs Wikisource / Commons / VIAF.
- Export Intuition on-chain.
- Requalification legacy → quality.
- Quota produit « ≥ 500 pins ».

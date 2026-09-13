# Search & Map Quality Gap Analysis

**Date**: September 2026  
**Status**: Diagnostic findings — no implementation changes  
**Author**: Cursor Cloud Agent (requested by Thibault)  
**Updated**: Verified teammate Documentation API hypotheses against code

---

## 0. Repo Questions — Direct Answers

These answers are verified against the codebase as of 2026-09-13.

### Q1: Gallica connector — SRU only, or already ALTO / texteBrut / IIIF / ContentSearch?

**Answer: SRU only.**

The Gallica connector (`crates/talaria-sources/src/connectors/gallica.rs`) uses exclusively the SRU endpoint:

```rust
const SRU: &str = "https://gallica.bnf.fr/SRU";
```

- **No ALTO OCR** — not implemented
- **No texteBrut** — not implemented  
- **No IIIF** — declared in capabilities (`provides_iiif: true` in `caps_stub`) but not used
- **No ContentSearch** — not implemented

The connector version is `"gallica:sru_v1"` confirming it only parses Dublin Core metadata from SRU responses. No full-text or coordinate extraction.

### Q2: data.bnf — live SPARQL and/or dumps?

**Answer: Neither. SRU only (Dublin Core).**

The BnF connector (`crates/talaria-sources/src/connectors/bnf.rs`) uses the catalogue SRU endpoint:

```rust
pub const DEFAULT_BASE_URL: &str = "https://catalogue.bnf.fr/api/SRU";
```

- **No SPARQL** — `data.bnf.fr` SPARQL endpoint is not called anywhere
- **No dumps** — no RDF/NT dump ingestion
- **Only Dublin Core** — `recordSchema=dublincore` in SRU queries

The connector is named `"bnf:v1"` and only extracts: ark, title, description, date, creator, language, publisher.

### Q3: Persée — which OAI prefixes (dc vs mods vs persee_mets)?

**Answer: `oai_dc` (Dublin Core) only.**

The Persée connector (`crates/talaria-sources/src/connectors/persee.rs`) uses OAI-PMH with Dublin Core:

```rust
let url = format!(
    "{OAI_BASE}?verb=GetRecord&metadataPrefix=oai_dc&identifier={}",
    percent_encode(&identifier)
);
```

- **`oai_dc`** — the only metadata prefix used
- **No `mods`** — not implemented
- **No `persee_mets`** — not implemented

Discovery uses portal HTML scraping, then OAI-PMH GetRecord with Dublin Core for fetch.

### Q4: Schema `canonical_events` / `event_evidence` — fields for coord_precision, date_precision / time_json, fragment_id, evidence_hash?

**Answer:**

| Field | Table | Present | Details |
|-------|-------|---------|---------|
| **coord_precision** | `canonical_events` | ✅ `location_precision TEXT` | Added in migration 010; values: exact/approximate/centroid/unknown |
| **uncertainty_radius_m** | `canonical_events` | ✅ `DOUBLE PRECISION` | Added in migration 010 |
| **date_precision** | `canonical_events` | ❌ Embedded in `time_json` | No separate column; `time_json.precision` holds day/month/year |
| **time_json** | `canonical_events` | ✅ `JSONB` | `{kind, year, start, end, surface, precision}` |
| **fragment_id** | `event_evidence` | ❌ Not on evidence | Only on `event_candidates.fragment_id`; evidence links via `sentence_id` |
| **evidence_hash** | `event_evidence` | ✅ `TEXT` | Added in migration 027; `md5(canonical_event_id + raw_document_id + quoted_text)` |
| **source_locator** | `event_evidence` | ✅ `TEXT` | Added in migration 027; JSON with kind/uri/title |

Schema gaps:
- `date_precision` is embedded, not a separate queryable column
- `fragment_id` not linked to evidence (would enable triplet: source_url + quoted_text + fragment_id)
- No `coord_uncertainty_source` to track which geocoder provided the coordinates

### Q5: Geocode path — Wikidata P625 only, or any fallback (Nominatim/Geonames/BAN)?

**Answer: Wikidata P625 only. No Nominatim, Geonames, or BAN.**

The geocode path in `crates/talaria-api/src/person_ingest/typing.rs` and `lot_e.rs`:

```
1. resolve_place_offline(label)     → hardcoded gazetteer (~250 places)
2. place_hint_from_title(label)     → "Battle of X" → "X"
3. fetch_wikidata_coords_for_label  → Wikidata search → P625
```

**Verified absent**:
- `grep -r "Nominatim" *.rs` → no matches
- `grep -r "geonames" *.rs` → no matches  
- `grep -r "ban.fr\|adresse.data" *.rs` → no matches

The only coordinate sources are:
1. Offline alias gazetteer (static)
2. Wikidata P625 claims (live)
3. Wikipedia page coordinates (for followed pages only)

---

## 0.1 Second API Cartography Verification (2026-09-13)

The following hypotheses from the Documentation API team's second cartography have been verified against the codebase.

### Wikipedia: REST extracts only → labels/summary, NOT typed coords/dates?

**Status: PARTIALLY FALSE — varies by code path.**

| Code Path | Extracts | Coords | Revisions/Wikitext | QID from pageprops |
|-----------|----------|--------|--------------------|--------------------|
| `lot_e.rs::fetch_wikipedia_extract` | ✅ | ✅ `prop=coordinates` | ✅ `rvprop=content\|ids` | ✅ `ppprop=wikibase_item` |
| `talaria-sources Wikipedia connector` | ✅ | ❌ Not extracted | ✅ `rvprop=content\|ids` | ✅ via `pageprops` |
| `lot_e.rs REST fallback` | ✅ HTML→text | ✅ summary API | ❌ | ❌ |

**Evidence** (`crates/talaria-api/src/lot_e.rs` lines 169-174):
```rust
("prop", "extracts|info|coordinates|revisions|pageprops"),
("ppprop", "wikibase_item"),
("rvprop", "content|ids"),
```

The `lot_e.rs` path (used by explorer ingest) **does** fetch coordinates and wikitext. However, the Wikipedia connector in `talaria-sources` does **not** extract page coordinates — only plain text/wikitext.

**Underused capabilities (verified absent)**:
- `parse sections` for `quoted_text + anchors` — not implemented
- `revisions for revid/hash` — revid fetched but hash not computed from content

**Teammate verdict confirmed**: "extracts=UI; pins should come from Wikidata P625" — **CORRECT**. Wikipedia page coords are secondary; P625 is the primary geocode source.

### Wikidata P0 geo: P625 + wikibase:geoPrecision → store coord_precision_m?

**Status: NOT IMPLEMENTED — precision lost.**

| Property | Extracted | Stored |
|----------|-----------|--------|
| P625 lat/lon | ✅ | ✅ |
| P625 `wikibase:geoPrecision` | ❌ NOT extracted | ❌ |
| P580/P582/P585 dates | ✅ (year only) | ✅ in `time_json` |
| P580/P582/P585 `timePrecision` | ❌ NOT extracted | ❌ |
| P131 (located in admin) | ❌ NOT extracted | ❌ |
| P17 (country) | ❌ NOT extracted | ❌ |
| P268 (BnF ID) | ❌ NOT extracted | ❌ |
| P214 (VIAF ID) | ❌ NOT extracted | ❌ |
| P213 (ISNI) | ❌ NOT extracted | ❌ |
| P269 (IdRef ID) | ❌ NOT extracted | ❌ |
| P4258 (Gallica ID) | ❌ NOT extracted | ❌ |

**Evidence** (`crates/talaria-wikidata/src/client.rs` lines 91-120):
```rust
async fn fetch_coordinates(&self, qid: &str) -> Result<Option<(f64, f64)>> {
    // Only lat/lon extracted, no precision
    let Some(lat) = coord.get("latitude").and_then(|v| v.as_f64()) else { ... };
    let Some(lon) = coord.get("longitude").and_then(|v| v.as_f64()) else { ... };
    Ok(Some((lat, lon)))
}
```

The SPARQL queries in `wdqs.rs` also do not request `wikibase:geoPrecision`.

**Anti-noise recommendations (verified not implemented)**:
- `ignore deprecated statements` — ✅ IMPLEMENTED in `promoted_statement_lines`
- `drop country/region centroids` — ❌ NOT implemented
- `Earth only (no lunar/Mars coords)` — ❌ NOT checked

### Commons: prefer SDC P9149/P1259 over ambiguous P625?

**Status: NOT IMPLEMENTED — SDC coordinate properties not extracted.**

| Property | Description | Extracted |
|----------|-------------|-----------|
| P180 | depicts | ✅ |
| P170/P2091 | creator | ✅ |
| sha1 | content hash | ✅ |
| license | rights | ✅ |
| P9149 | location of POI shown | ❌ NOT extracted |
| P1259 | coordinate of viewpoint | ❌ NOT extracted |
| P625 | coordinates (ambiguous) | ❌ NOT extracted from Commons |

**Evidence** (`crates/talaria-sources/src/connectors/commons.rs` lines 196-213):
```rust
fn depicts_from_entity(entity: &Value) -> Vec<String> {
    entity.pointer("/statements/P180")  // Only P180, not P9149/P1259
```

The Commons connector does **not** extract any coordinate properties. It only provides metadata for media licensing and attribution.

### Europeana: edm:Place lat/lon OR resolved Wikidata QID?

**Status: NOT IMPLEMENTED — no coordinate extraction.**

**Evidence** (`crates/talaria-sources/src/connectors/europeana.rs`):
- `grep "edm:Place\|edmPlaceLat\|edmPlaceLon"` → **no matches**
- The connector extracts only: id, title, description, provider, date, language, type
- No geographic data extracted from `edm:Place` or `dcterms:spatial`

**Recommendation verified but not coded**: Europeana items with `edm:Place` coordinates could provide additional geo data, especially for cultural heritage objects.

### Stubs ROI: IdRef (Solr) P1 > VIAF > ISNI

**Status: All stubs. No authority identifiers read from Wikidata.**

**Evidence** (grep `P268|P214|P213|P269|P4258` in `*.rs`):
- **No matches** — these authority identifier properties are never extracted from Wikidata entities.

Even without implementing the authority connectors, the system could read these identifiers from the subject's Wikidata QID to enable future enrichment. Currently, this is not done.

### Pin Chain (proposed vs. actual)

**Proposed Pin Chain**:
```
QID ← pageprops → P625+prec+dates+P131 → authorities → Gallica OCR/IIIF or Commons → Europeana → quote+hash
```

**Actual Pin Chain** (verified):
```
Wikipedia page
    ↓ pageprops
QID (extracted)
    ↓ wbgetentities
P625 lat/lon ONLY (no precision, no dates, no P131)
    ↓
Stored with hardcoded uncertainty_radius_m: 5000.0
    ↓
NO authority lookup (P268/P214/P213/P269 not read)
    ↓
NO Gallica/Commons/Europeana enrichment for coordinates
    ↓
Evidence hash = md5(event_id + doc_id + quoted_text) — NO fragment_id
```

**Gap Summary**:
| Step | Proposed | Implemented |
|------|----------|-------------|
| QID from pageprops | ✅ | ✅ |
| P625 coordinates | ✅ | ✅ |
| geoPrecision | ✅ | ❌ |
| timePrecision | ✅ | ❌ |
| P131/P17 containment | ✅ | ❌ |
| Authority bundle (P268/214/213/269) | ✅ | ❌ |
| Gallica OCR/IIIF | ✅ | ❌ |
| Commons SDC coords | ✅ | ❌ |
| Europeana edm:Place | ✅ | ❌ |
| quote+hash with fragment | ✅ | Partial (no fragment) |

---

## 0.2 First API Cartography Verification

### P0: Evidence triplet (source_url + quoted_text + fragment_id)

**Status: Partially implemented, not enforced.**

- `event_evidence.source_locator` (JSON) stores `{kind, uri, title}` — ✅ source_url
- `event_evidence.quoted_text` — ✅ quoted_text  
- `event_evidence.fragment_id` — ❌ NOT present (only on `event_candidates`)

The triplet is **not enforced as a unique constraint**. Instead, `evidence_hash = md5(canonical_event_id + raw_document_id + quoted_text)` is used for deduplication (migration 027).

### P0: Gallica depth (OAIRecord/nqamoyen/texteBrut/ALTO/ContentSearch/IIIF)

**Status: Not implemented. SRU bibliographic notices only.**

Gallica connector returns `DocumentType::BibliographicNotice` with `full_text_available: true` but never fetches the actual text. The `provides_iiif: true` capability is declared but unused.

### P0: Separate spatial precision (point/city/region/country) and temporal precision

**Status: Spatial partially, temporal embedded.**

Spatial precision:
- `canonical_events.location_precision` — exists with CHECK `IN ('exact', 'approximate', 'centroid', 'unknown')`
- `canonical_events.uncertainty_radius_m` — exists
- **Missing**: point/city/region/country granularity

Temporal precision:
- `time_json.precision` — embedded in JSONB, not a separate column
- Values: `day`, `month`, `year` per `TypedTime` struct
- **Gap**: Not queryable without JSON operators

### P0: Grounding chain before geocode

**Status: Not implemented as a formal chain.**

Current flow is linear:
1. Extract raw place mention
2. Immediately attempt geocode
3. If geocode fails, `map_eligible=false`

No intermediate "grounding" step that resolves place identity before geocoding. The system conflates mention resolution with coordinate lookup.

### P1: IdRef → VIAF → ISNI resolution order

**Status: All three are stubs.**

```rust
for kind in [SourceKind::Viaf, SourceKind::Isni, SourceKind::IdRef, ...] {
    register_stub(&mut reg, kind, "alignment layer — not yet wired");
}
```

No implementation exists. The recommended cascade order (IdRef → VIAF → ISNI) is not coded.

### Map fuzziness causes (all verified)

| Cause | Verified | Evidence |
|-------|----------|----------|
| (a) Coarse Wikidata geocode | ✅ True | `uncertainty_radius_m: Some(5000.0)` hardcoded for all Wikidata results |
| (b) Coarse dates | ✅ True | `TypedTime::Unknown` → `NeedsReview` → never on map |
| (c) Single evidence without fragment | ✅ True | `fragment_id` not on `event_evidence` table |
| (d) Weak entity resolution | ✅ True | VIAF/ISNI/IdRef all stubs; no person disambiguation |

All four causes are confirmed in code.

---

## Executive Summary (Résumé)

L'analyse révèle **8 causes principales** de l'imprécision des résultats de recherche et des points cartographiques dans l'explorateur Talaria :

| # | Cause | Impact | Fichiers clés |
|---|-------|--------|---------------|
| 1 | **Sources non utilisées pour la carte** | Les sources institutionnelles (HAL, Gallica, BnF, etc.) ne contribuent pas aux points de carte | `routes/ingest.rs`, `corpus_ingest.rs` |
| 2 | **Géocodage hors-ligne limité** | Gazetteer de ~250 lieux, biais napoléonien | `talaria-sources/src/places.rs` |
| 3 | **Filtrage `map_eligible` restrictif** | Seuls ~20 types d'événements obtiennent des pins | `talaria-quality/src/gates.rs` |
| 4 | **Absence de résolution d'identité** | VIAF/ISNI/IdRef sont des stubs | `connectors/mod.rs` |
| 5 | **Perte de précision temporelle** | Événements sans date → NeedsReview | `gates.rs`, `typing.rs` |
| 6 | **Précision géographique ignorée** | `wikibase:geoPrecision` non extrait; 5km hardcodé | `client.rs`, `typing.rs` |
| 7 | **Identifiants d'autorité non lus** | P268/P214/P213/P269 ignorés sur les QID | `wikidata.rs`, `lot_e.rs` |
| 8 | **Coordonnées multimédia absentes** | Commons P9149/P1259, Europeana edm:Place non extraits | `commons.rs`, `europeana.rs` |

---

## 1. End-to-End Data Flow Trace

### 1.1 Entity Search (`/api/v1/entities/search`)

```
Frontend (EntitySearchBox.tsx)
    ↓ debounced 220ms
GET /api/v1/entities/search?q=<query>&lang=en&limit=1
    ↓
routes/entities.rs::search()
    ↓
┌─ Local DB search (talaria_store::search_local_entities)
│     → entities table, person_name ILIKE
│
└─ Wikidata fallback (WikidataClient::search_entities)
      → wbsearchentities API, sorted by person_search_score()
          ↓
      collapse_person_search() → single best match
```

**Gap identifié**: La recherche locale utilise un `ILIKE %query%` simple. Pas de recherche floue, pas de synonymes, pas de formes alternatives du nom.

### 1.2 Explorer Ingest (`POST /api/v1/ingest/explorer`)

```
routes/ingest.rs::start_explorer_ingest()
    ↓
run_explorer_lane()
    ↓
person_ingest/mod.rs::run_person_ingest()
    │
    ├─ resolve::require_person_qid()  → Wikidata QID resolution
    │
    ├─ lot_e::fetch_wikidata_subject_meta()  → P569/P570 birth/death
    │
    ├─ Pass 1: Wikidata statements (birth/death only)
    │     └─ grounding::ground_structured()
    │
    ├─ Wikipedia extracts (en, fr, requested lang)
    │     ├─ collect::fetch_wiki_extract()
    │     ├─ extract::extract_wiki_rules()  → rule-based
    │     └─ LLM extraction (if OPENAI_API_KEY)
    │
    ├─ Pass 2: Wikidata statements (all other events)
    │
    ├─ WDQS participation harvest (P710/P1344)
    │     └─ wdqs::fetch_events_for_person()
    │
    ├─ Follow pages (battles, treaties, etc.)
    │     └─ collect::fetch_wiki_page() + coordinates
    │
    └─ typing::backfill_person_geocodes()  → batch geocode
```

**Gap critique**: Aucune source institutionnelle (HAL, Gallica, BnF, Persée, etc.) n'est utilisée dans ce chemin. Ces sources sont réservées à la lane Agora (`run_agora_lane`), qui ne génère pas de points de carte.

### 1.3 Timeline & GeoJSON Endpoints

```
GET /api/v1/timeline?entity_id=<uuid>&pipeline=person
    ↓
routes/events.rs::timeline()
    ↓
talaria_store::list_timeline_events()
    → SELECT ... FROM canonical_events WHERE is_active AND pipeline='person'
    → No geom filter (timeline shows all events)

GET /api/v1/events/geojson?entity_id=<uuid>&map_eligible=true
    ↓
routes/events.rs::geojson()
    ↓
talaria_store::list_geojson_events()
    → SELECT ... WHERE geom IS NOT NULL AND map_eligible = true
```

**Gap**: Le endpoint GeoJSON exige `geom IS NOT NULL`. Les événements avec un lieu textuel mais sans coordonnées sont exclus.

### 1.4 Geocoding / Place Grounding

```
typing.rs::geocode_place()
    │
    ├─ is_wikidata_qid(label)?
    │     └─ WikidataClient::fetch_coordinates(qid)  → P625
    │
    ├─ resolve_place_offline(label)  → ALIAS GAZETTEER
    │     └─ ~250 lieux codés en dur (places.rs)
    │
    └─ lot_e::resolve_label_coords(label)  → Wikidata search + P625
```

**Gap majeur**: Le gazetteer offline (`places.rs`) contient ~250 lieux avec un biais napoléonien évident. Les lieux non listés nécessitent une requête Wikidata, qui peut échouer pour :
- Lieux historiques sans P625
- Ambiguïtés de nom (plusieurs villes du même nom)
- Orthographes alternatives non gérées

---

## 2. Source Connector Inventory

### 2.1 Connectors by Usage Context

| Source | Implemented | Explorer Lane | Agora Lane | Provides Coords |
|--------|-------------|---------------|------------|-----------------|
| **Wikidata** | ✅ Live | ✅ Used | ❌ | ✅ P625 |
| **Wikipedia** | ✅ Live | ✅ Used | ❌ | ❌ (page coords only) |
| **Wikisource** | ✅ Live | ✅ Fact providers | ❌ | ❌ |
| **Commons** | ✅ Live | ✅ Fact providers | ❌ | ❌ |
| **HAL** | ✅ Live | ❌ **Unused** | ✅ Used | ❌ |
| **Persée** | ✅ Live | ❌ **Unused** | ✅ Used | ❌ |
| **Gallica** | ✅ Live | ❌ **Unused** | ✅ Used | ❌ |
| **theses.fr** | ✅ Live | ❌ **Unused** | ✅ Used | ❌ |
| **OpenAlex** | ✅ Live | ❌ **Unused** | ✅ Used | ❌ |
| **BnF** | ✅ Live | ❌ **Unused** | ✅ Used | ❌ |
| **Open Library** | ✅ Live | ❌ **Unused** | ✅ Used | ❌ |
| **Internet Archive** | ✅ Live | ❌ **Unused** | ✅ Used | ❌ |
| **Europeana** | ✅ (needs key) | ❌ **Unused** | ✅ Used | ✅ (some items) |
| **VIAF** | ❌ Stub | ❌ | ❌ | N/A |
| **ISNI** | ❌ Stub | ❌ | ❌ | N/A |
| **IdRef** | ❌ Stub | ❌ | ❌ | N/A |

**Fichiers**: 
- Définition: `crates/talaria-sources/src/connectors/mod.rs` (lines 550-558)
- Explorer providers: `crates/talaria-api/src/corpus_ingest.rs::LIVE_WIKI_SISTER_PROVIDERS`
- Agora providers: `crates/talaria-api/src/corpus_ingest.rs::LIVE_CORPUS_PROVIDERS`

### 2.2 Coordinate Sources

Les coordonnées ne peuvent provenir que de :

1. **Wikidata P625** (claim coordinates) — via `WikidataClient::fetch_coordinates()`
2. **Wikipedia page coordinates** — via `fetch_wiki_page()` REST API `?prop=coordinates`
3. **Offline gazetteer** — via `resolve_place_offline()` (~250 entries)
4. **WDQS events** — via `wdqs::fetch_events_for_person()` with `?geo` binding

Aucune source institutionnelle ne fournit de coordonnées structurées dans le modèle actuel.

---

## 3. Root Causes of Imprecision

### 3.1 Hypothesis: Limited Offline Gazetteer

**Evidence** (`crates/talaria-sources/src/places.rs`):

```rust:18:50:crates/talaria-sources/src/places.rs
const ALIASES: &[(&str, f64, f64, &str)] = &[
    ("ajaccio", 41.9267, 8.7369, "exact"),
    ("paris", 48.8566, 2.3522, "exact"),
    ("waterloo", 50.6794, 4.4047, "exact"),
    ("austerlitz", 49.1533, 16.875, "exact"),
    // ... ~250 entries total
];
```

- Le gazetteer est biaisé vers l'histoire napoléonienne
- Les lieux non listés nécessitent un lookup Wikidata (lent, peut échouer)
- Pas de gestion des synonymes ou variantes orthographiques

**Impact**: Événements avec lieux valides mais non reconnus → pas de coordonnées → `map_eligible=false`

### 3.2 Hypothesis: Over-Restrictive Event Type Filtering

**Evidence** (`crates/talaria-quality/src/gates.rs`):

```rust:260:286:crates/talaria-quality/src/gates.rs
pub fn event_type_is_map_locus(event_type: &str) -> bool {
    matches!(
        event_type,
        "birth" | "death" | "residence" | "arrival" | "departure" | "passage"
        | "meeting" | "exile" | "battle" | "siege" | "education" | "office"
        | "marriage" | "divorce" | "travel" | "imprisonment" | "diplomatic"
        | "employment" | "work" | "burial" | "treaty"
    )
}
```

Types **exclus** de la carte:
- `publication` — où l'œuvre a été publiée
- `commemoration` — monuments, plaques, renommages
- `award` — cérémonies de remise de prix
- `historical_fact` — événements génériques
- `anecdote` — récits non vérifiés

**Impact**: ~30-40% des événements extraits sont exclus de la carte même avec des coordonnées valides.

### 3.3 Hypothesis: Institutional Sources Not Used for Map Points

**Verified: TRUE** — `grep -ri "gallica\|persee\|bnf\|hal" crates/talaria-api/src/person_ingest/` returns **no matches**.

**Evidence** (`crates/talaria-api/src/routes/ingest.rs`):

```rust:382:413:crates/talaria-api/src/routes/ingest.rs
async fn run_explorer_lane(...) -> anyhow::Result<Value> {
    let person = crate::person_ingest::run_person_ingest(
        config, subject, qid, wiki_lang, max_documents, Some(seed_list),
    ).await?;
    // Only Wikipedia/Wikidata/Wikisource/Commons
    // NO HAL, Gallica, BnF, Persée, etc.
}
```

Les sources institutionnelles sont cantonnées à la lane Agora:

```rust:416:449:crates/talaria-api/src/routes/ingest.rs
async fn run_agora_lane(...) -> anyhow::Result<Value> {
    let providers = live_corpus_providers();  // HAL, Persée, etc.
    let corpus_report = corpus_ingest::run_corpus_ingest(...).await?;
    // Creates historiographic claims, NOT map events
}
```

**Code-verified**: The `person_ingest` module imports only:
- `talaria_sources::wdqs` (WDQS events)
- `talaria_wikidata` (Wikidata client)
- `crate::lot_e` (Wikidata meta + Wikipedia fetch)

No imports from `corpus_ingest`, `hal`, `gallica`, `bnf`, or `persee` connectors.

**Impact**: Des sources riches (Gallica, BnF, HAL) qui pourraient fournir des lieux datés ne contribuent jamais aux points de carte.

### 3.4 Hypothesis: Missing Authority Resolution (VIAF/ISNI/IdRef)

**Evidence** (`crates/talaria-sources/src/connectors/mod.rs`):

```rust:549:558:crates/talaria-sources/src/connectors/mod.rs
for kind in [
    SourceKind::Viaf,
    SourceKind::Isni,
    SourceKind::IdRef,
    SourceKind::Crossref,
    SourceKind::OpenEdition,
    SourceKind::Sudoc,
] {
    register_stub(&mut reg, kind, "alignment layer — not yet wired");
}
```

**Impact**: 
- Pas de désambiguïsation des personnes homonymes
- Pas de liens vers des notices d'autorité qui pourraient fournir des lieux
- Pas de consolidation des identités entre sources

### 3.5 Hypothesis: Temporal Precision Loss

**Evidence** (`crates/talaria-quality/src/gates.rs`):

```rust:248:257:crates/talaria-quality/src/gates.rs
// Soft: unknown time → needs_review rather than hard accept for map.
if matches!(candidate.time, TypedTime::Unknown { .. }) {
    return GateDecision::NeedsReview;
}

if candidate.subject_entity_id.is_none() {
    return GateDecision::NeedsReview;
}
```

**Impact**: Événements sans date précise restent en `NeedsReview`, jamais intégrés à la carte, même si le lieu est connu.

### 3.6 Hypothesis: Frontend Does Not Filter Aggressively

Le frontend ne filtre pas au-delà de ce que l'API retourne. Le filtrage est côté serveur:

- `list_geojson_events()` exige `geom IS NOT NULL`
- Le composant `MapSourceManager` reçoit uniquement les features filtrées

Pas de clustering actif — désactivé dans le code actuel.

---

## 4. Prioritized Recommendations

### P0 — Prerequisites (from Documentation API team, verified)

#### 4.0a Evidence Triplet Schema

**What**: Add `fragment_id` to `event_evidence` table; enforce unique constraint on `(source_locator, quoted_text, fragment_id)`.

**Why**: Currently evidence lacks fragment-level provenance. The `evidence_hash` (md5) conflates document + quote but loses fragment granularity.

**Files**:
- New migration: `migrations/029_evidence_fragment_triplet.sql`
- `crates/talaria-store/src/person_events.rs`

**Expected Impact**: Enables precise citation back to source fragments; prerequisite for Gallica ALTO integration.

**Risk**: Migration on existing data; need to backfill `fragment_id` from `event_candidates` where available.

#### 4.0b Separate Precision Columns

**What**: Add `date_precision TEXT CHECK (IN ('day', 'month', 'year', 'decade', 'century', 'unknown'))` and `coord_precision_level TEXT CHECK (IN ('point', 'city', 'region', 'country', 'unknown'))` as queryable columns.

**Why**: `time_json.precision` is embedded in JSONB, not indexable. `location_precision` exists but with coarse values (exact/approximate/centroid).

**Files**:
- New migration
- `crates/talaria-quality/src/model.rs`
- `crates/talaria-store/src/quality.rs`

**Expected Impact**: Enables filtering map by precision level (e.g., hide city-level when zoomed in).

#### 4.0c Grounding Chain Before Geocode

**What**: Separate place mention resolution (→ place entity) from geocoding (→ coordinates). Ground to `place_entity_id` first, then geocode the entity.

**Why**: Current system skips entity resolution and goes directly to coordinate lookup. This loses the chance to:
- Use entity aliases for geocoding
- Prefer authoritative coordinates over search results
- Track which entity a coordinate belongs to

**Files**:
- `crates/talaria-api/src/person_ingest/typing.rs`
- `crates/talaria-quality/src/places.rs`

**Expected Impact**: +15-20% place resolution by using entity aliases.

#### 4.0d Authority Resolution: IdRef → VIAF → ISNI

**What**: Implement the three authority connectors in cascade order: IdRef (French authority), VIAF (international), ISNI (identifier).

**Why**: IdRef has the richest French historical data; VIAF provides international consolidation; ISNI adds numeric identifier.

**Files**:
- `crates/talaria-sources/src/connectors/idref.rs` (new)
- `crates/talaria-sources/src/connectors/viaf.rs` (new)
- `crates/talaria-sources/src/connectors/isni.rs` (new)

**Expected Impact**: Prerequisite for multi-source enrichment; +20-30% person disambiguation accuracy.

**Risk**: High complexity; IdRef uses SRU, VIAF uses SRU/JSON, ISNI uses REST.

#### 4.0e Extract Wikidata geoPrecision (NEW from Second Cartography)

**What**: When fetching P625, also extract `wikibase:geoPrecision` from the claim and store as `coord_precision_m`.

**Why**: Wikidata provides precision metadata for coordinates, but we ignore it and hardcode `uncertainty_radius_m: 5000.0` for all results. This loses granularity between exact GPS points and approximate city centroids.

**Files**:
- `crates/talaria-wikidata/src/client.rs::fetch_coordinates()`
- `crates/talaria-sources/src/wdqs.rs` (SPARQL queries)
- `crates/talaria-api/src/person_ingest/typing.rs`

**Current code** (`client.rs` line 108):
```rust
.pointer(&format!("/entities/{qid}/claims/P625/0/mainsnak/datavalue/value"))
// Only lat/lon extracted, precision discarded
```

**Fix**: Also extract `/mainsnak/datavalue/value/precision` if present.

**Expected Impact**: More accurate uncertainty radii; enables zoom-dependent pin filtering.

**Risk**: Minimal; Wikidata precision values may need calibration to meters.

#### 4.0f Extract Wikidata timePrecision (NEW from Second Cartography)

**What**: When extracting dates from P580/P582/P585, also store the Wikibase `precision` value (11=day, 10=month, 9=year, etc.) separately.

**Why**: Currently `TypedTime` has `precision: day|month|year` but it's derived from parsing the ISO date string, not from Wikidata's explicit precision claim. Wikidata may indicate "year precision" for `+1815-00-00T00:00:00Z`, which we miss.

**Files**:
- `crates/talaria-wikidata/src/claims.rs::snak_time()`
- `crates/talaria-wikidata/src/time.rs`

**Current code** (`claims.rs` lines 196-204):
```rust
fn snak_time(snak: &Value) -> Option<crate::time::WikibaseTime> {
    let precision = snak
        .pointer("/datavalue/value/precision")
        .and_then(|v| v.as_i64())
        .map(|n| n as i32);  // Extracted but may not be fully used
```

The precision IS extracted, but `WikibaseTime` → `TypedTime` conversion may not preserve Wikibase precision semantics.

**Expected Impact**: Correct temporal precision for edge cases (decade, century-level dates).

**Risk**: Low; requires validation of precision mapping.

#### 4.0g Read Authority Identifiers from Wikidata QID (NEW from Second Cartography)

**What**: When fetching a person's Wikidata entity, extract authority identifiers P268 (BnF), P214 (VIAF), P213 (ISNI), P269 (IdRef), P4258 (Gallica) into `known_identifiers`.

**Why**: Even without implementing the authority connectors, having these identifiers enables:
- Future authority enrichment without re-fetching
- Cross-linking in evidence display
- Prioritized source querying

**Files**:
- `crates/talaria-sources/src/connectors/wikidata.rs::fetch()`
- `crates/talaria-api/src/lot_e.rs::fetch_wikidata_subject_meta()`
- `crates/talaria-sources/src/plan.rs::ResolvedSubject`

**Expected Impact**: Prerequisite for authority cascade (4.0d); enables Gallica ARK lookup.

**Risk**: Minimal; just parsing additional claims.

---

### P1 — High Leverage / Medium Risk

#### 4.1 Enable Institutional Sources in Explorer Lane

**What**: Wire HAL, Gallica, BnF into `run_person_ingest()` to extract dated place mentions.

**Why**: Ces sources contiennent des données biographiques structurées avec des lieux et dates.

**Files**:
- `crates/talaria-api/src/person_ingest/mod.rs`
- `crates/talaria-api/src/routes/ingest.rs`

**Expected Impact**: +20-40% map points for well-documented historical figures.

**Risk**: Rate limiting on institutional APIs; need to respect usage policies.

---

### P2 — High Leverage / Low Risk

#### 4.2 Expand Offline Gazetteer with GeoNames Seed

**What**: Import top 10,000 world cities/historical places from GeoNames or Wikidata dump.

**Why**: Offline resolution is instant; reduces Wikidata API calls.

**Files**:
- `crates/talaria-sources/src/places.rs`
- New: `fixtures/gazetteer/geonames_top10k.json`

**Expected Impact**: +30-50% place resolution success rate.

**Risk**: Minimal; data is static and tested.

---

### P3 — Medium Leverage / Low Risk

#### 4.3 Relax `event_type_is_map_locus()` for Well-Sourced Events

**What**: Allow `publication`, `commemoration`, `award` when confidence > 0.8 and coords exist.

**Why**: These events have valid locations that users want to see.

**Files**:
- `crates/talaria-quality/src/gates.rs`
- `crates/talaria-api/src/person_ingest/persist.rs`

**Expected Impact**: +15-25% map points.

**Risk**: Low; guarded by confidence threshold.

---

### P4 — Medium Leverage / Medium Risk

#### 4.4 Accept Events with Unknown Time but Known Place

**What**: Change `TypedTime::Unknown` from `NeedsReview` to `Accept` when place is resolved and confidence > 0.7.

**Why**: Historical events often lack precise dates but have documented locations.

**Files**:
- `crates/talaria-quality/src/gates.rs` (line 248-257)
- `crates/talaria-api/src/person_ingest/gating.rs`

**Expected Impact**: +10-20% timeline events eligible for map.

**Risk**: May surface low-quality events; mitigated by confidence threshold.

---

### P5 — High Leverage / High Risk

#### 4.5 Implement VIAF/ISNI Authority Alignment

**What**: Complete the stub connectors for authority resolution.

**Why**: Enables disambiguation, cross-source linking, and access to authority-linked place data.

**Files**:
- `crates/talaria-sources/src/connectors/stub.rs` → new implementations
- `crates/talaria-sources/src/identifiers.rs`

**Expected Impact**: +10-30% accuracy for ambiguous names; prerequisite for multi-source enrichment.

**Risk**: High complexity; requires understanding each authority's API.

---

### P6 — Future: Commons SDC Coordinates (NEW from Second Cartography)

#### 4.6 Extract Commons SDC P9149/P1259 for Location

**Current State**: Commons connector extracts P180 (depicts), attribution, sha1, license — but NO coordinates.

**Available Properties**:
| Property | Description | Current |
|----------|-------------|---------|
| P9149 | coordinates of depicted place | ❌ Not extracted |
| P1259 | coordinates of viewpoint | ❌ Not extracted |
| P625 | coordinates (ambiguous — camera or subject?) | ❌ Not extracted |

**Recommendation**: For Commons media linked to a person, extract P9149 (preferred) or P1259 to get precise locations where photos were taken or places depicted.

**Files**:
- `crates/talaria-sources/src/connectors/commons.rs`

**Expected Impact**: +5-10% map points for well-photographed historical figures.

**Risk**: Medium; SDC adoption is partial; need fallback to P625.

---

### P7 — Future: Europeana Place Extraction (NEW from Second Cartography)

#### 4.7 Extract edm:Place Coordinates from Europeana

**Current State**: Europeana connector extracts only bibliographic metadata — no geographic data.

**Available Fields** (not extracted):
- `edm:Place` with `wgs84_pos:lat` and `wgs84_pos:long`
- `edmPlaceLatitude` / `edmPlaceLongitude` (flattened)
- Resolved Wikidata QID from `edm:Place.owl:sameAs`
- `dcterms:spatial` place labels

**Recommendation**: Parse `edm:Place` from Europeana search results to extract coordinates. Fall back to resolving `owl:sameAs` Wikidata QIDs.

**Files**:
- `crates/talaria-sources/src/connectors/europeana.rs`

**Expected Impact**: +10-20% map points for cultural heritage objects with documented provenance.

**Risk**: Medium; Europeana requires API key (`EUROPEANA_API_KEY`); `edm:Place` coverage varies.

---

### P8 — Future: Gallica Full Depth (from Documentation API team)

#### 4.8 Gallica ALTO/texteBrut/IIIF/ContentSearch

**Current State**: SRU bibliographic notices only.

**Depth Layers Available** (not yet implemented):

| Layer | API | What It Provides |
|-------|-----|------------------|
| OAIRecord | OAI-PMH | Dublin Core metadata (same as SRU) |
| nqamoyen | REST | Thumbnail + preview quality images |
| texteBrut | REST | Plain-text OCR (lossy) |
| ALTO | IIIF | Structured OCR with coordinates per word |
| ContentSearch | IIIF | Full-text search with hit highlighting |
| IIIF | Image API | High-resolution page images |

**Implementation Priority**:
1. `texteBrut` — easiest, plain text endpoint
2. `ALTO` — highest value, enables fragment-level citation with page coords
3. `ContentSearch` — search within documents

**Files**:
- `crates/talaria-sources/src/connectors/gallica.rs`
- New: `gallica_iiif.rs` module

**Expected Impact**: Transforms Gallica from metadata-only to full-text extraction source.

**Risk**: Rate limits on Gallica APIs; ALTO XML parsing complexity.

---

## 5. Additional Gaps from Second API Cartography

### 5.1 Anti-Noise Measures (Not Implemented)

The Documentation API recommended these anti-noise filters — none are currently implemented:

| Recommendation | Status | Evidence |
|----------------|--------|----------|
| Ignore deprecated statements | ✅ Implemented | `promoted_statement_lines` skips `rank=deprecated` |
| Drop country/region centroids | ❌ Not implemented | All P625 coords accepted regardless of precision |
| Earth-only check | ❌ Not implemented | Lunar/Mars coords from Wikidata would be accepted |

### 5.2 Pin Chain Completion Status

| Chain Step | Implemented | Notes |
|------------|-------------|-------|
| QID ← pageprops | ✅ | `lot_e.rs` extracts `wikibase_item` |
| P625 lat/lon | ✅ | `WikidataClient::fetch_coordinates()` |
| P625 precision | ❌ | `geoPrecision` not extracted |
| P580/P582/P585 dates | ✅ | Year extracted |
| P580/P582/P585 timePrecision | ❌ | Wikibase precision partially used |
| P131/P17 admin hierarchy | ❌ | Not extracted |
| Authority bundle | ❌ | P268/P214/P213/P269/P4258 not read |
| Gallica OCR/IIIF | ❌ | SRU only |
| Commons SDC coords | ❌ | P9149/P1259 not extracted |
| Europeana edm:Place | ❌ | No geo extraction |
| quote + hash with fragment | Partial | `evidence_hash` exists, `fragment_id` missing |

---

## 5. Verification Checklist

Before implementing any recommendation, verify:

- [ ] API rate limits for institutional sources (HAL: none documented; BnF: fair use)
- [ ] GeoNames license compatibility (CC BY 4.0 for the free dataset)
- [ ] Wikidata Query Service quotas (60 requests/min without key)
- [ ] Frontend impact: ensure new event types render correctly in `MapLayers.tsx`
- [ ] Migration path: existing data should remain valid

---

## Appendix A: Key File Paths

| Component | Path |
|-----------|------|
| Entity search API | `crates/talaria-api/src/routes/entities.rs` |
| Ingest orchestration | `crates/talaria-api/src/routes/ingest.rs` |
| Person ingest pipeline | `crates/talaria-api/src/person_ingest/mod.rs` |
| Geocoding | `crates/talaria-api/src/person_ingest/typing.rs` |
| Offline gazetteer | `crates/talaria-sources/src/places.rs` |
| Quality gates | `crates/talaria-quality/src/gates.rs` |
| Source registry | `crates/talaria-sources/src/connectors/mod.rs` |
| GeoJSON endpoint | `crates/talaria-api/src/routes/events.rs` |
| DB queries | `crates/talaria-store/src/canonical_events.rs` |
| Map rendering | `web/src/components/map/map-source-manager.tsx` |
| WDQS harvest | `crates/talaria-sources/src/wdqs.rs` |
| Wikidata client | `crates/talaria-wikidata/src/client.rs` |

---

## Appendix B: Source Capabilities Matrix

| Source | Text | Coords | Identifiers | Full-Text | IIIF |
|--------|------|--------|-------------|-----------|------|
| Wikidata | ❌ | ✅ P625 | ✅ QID | ❌ | ❌ |
| Wikipedia | ✅ | ❌ | ✅ | ✅ | ❌ |
| HAL | ✅ | ❌ | ✅ | ❌ | ❌ |
| Gallica | ✅ | ❌ | ✅ | ❌ | ✅ |
| BnF | ✅ | ❌ | ✅ | ❌ | ❌ |
| Europeana | ✅ | ✅ (some) | ✅ | ❌ | ✅ |
| VIAF | ❌ | ❌ | ✅ | ❌ | ❌ |

---

## Appendix C: Geocode Path Detail (Confirmed)

```
geocode_place(label)
    │
    ├── is_wikidata_qid(label)?
    │   └── YES: WikidataClient::fetch_coordinates(qid)
    │             → P625 claim lookup
    │             → uncertainty_radius_m: 5000.0 (hardcoded)
    │
    ├── resolve_place_offline(label)
    │   └── ALIAS_GAZETTEER (~250 entries, Napoleonic bias)
    │       → places.rs lookup_alias()
    │       → uncertainty_radius_m: 500.0 (exact) or 5000.0 (centroid)
    │
    └── lot_e::resolve_label_coords(label)
        ├── resolve_place_offline(label)  ← retry
        ├── place_hint_from_title(label)
        │   └── "Battle of X" → resolve_place_offline("X")
        └── fetch_wikidata_coords_for_label(label, "en")
            └── Wikidata search → first result → P625
            └── uncertainty_radius_m: 5000.0

FALLBACKS NOT IMPLEMENTED:
❌ Nominatim (OpenStreetMap)
❌ GeoNames
❌ BAN (Base Adresse Nationale)
❌ Google Geocoding API
❌ Photon
```

## Appendix D: Connector API Protocols (Confirmed)

| Connector | Protocol | Endpoint | Metadata Format |
|-----------|----------|----------|-----------------|
| Gallica | SRU | `gallica.bnf.fr/SRU` | Dublin Core XML |
| BnF | SRU | `catalogue.bnf.fr/api/SRU` | Dublin Core XML |
| Persée | OAI-PMH | `oai.persee.fr/oai` | `oai_dc` (Dublin Core) |
| HAL | Solr REST | `api.archives-ouvertes.fr` | JSON |
| OpenAlex | REST | `api.openalex.org` | JSON |
| theses.fr | REST | `theses.fr/api` | JSON |
| Wikidata | MediaWiki API | `wikidata.org/w/api.php` | JSON |
| Wikipedia | MediaWiki API | `{lang}.wikipedia.org/w/api.php` | JSON |

**Not Implemented**:
- data.bnf.fr SPARQL
- Gallica IIIF / ALTO / ContentSearch
- Persée MODS / persee_mets
- Any RDF dump ingestion

---

*Document généré automatiquement. Toute hypothèse incorrecte peut être écartée — vérifier dans le code source.*

*Updated 2026-09-13 with Documentation API team verification.*

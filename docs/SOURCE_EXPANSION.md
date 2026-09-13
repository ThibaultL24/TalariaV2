# Source expansion Vague A/B

Product-truth notes for institutional / catalog sources (PR #20).

## Contracts (do not break)

- **Explorer** (map / timeline) ≠ **Agora** (catalogs / opinions).
- Extra sources = **evidence** on an existing `occurrence_key` — **never** auto-create a new map pin.
- `map_eligible` only when geom is resolved; otherwise timeline-only.
- TypedTime keeps **kind × precision**; SQL columns (`date_precision`, `coord_precision_level`) are **projections**, not a second model.
- Explorer ingest stays `pipeline='person'` only.

## Grounding order

mention → place identity (TGN / WHG stubs OK) → geocode (P625 / gazetteer / page coords).
TGN/WHG alone ≠ map pin. No Nominatim (or similar) invention without a traced coordinate source.

## Evidence triplet

Every reinforced fact needs:

- `source_locator`
- `quoted_text`
- `fragment_id`

Catalog hits without a stable fragment stay metadata / Agora — not explorer pins.

## Per-source doc table (fill for each connector in this PR)

| Field | Content |
|-------|---------|
| Protocol | SRU / OAI / SPARQL / REST / dump |
| Lane | Explorer evidence / Agora / both |
| Geo | Coord fields or QID/P625; else "no pin" |
| Time | Date fields + expected precision |
| Maturity | stub / metadata-only / full-text / extraction_ready |
| `source-status` | Ops-facing status string |

---

### Vague A: Corpus Evidence Sources

#### HAL (French Open Archive)

| Field | Content |
|-------|---------|
| Protocol | REST API (api.archives-ouvertes.fr) |
| Lane | Explorer evidence |
| Geo | No pin (affiliation text only) |
| Time | `publicationDate` — year precision typical |
| Maturity | extraction_ready |
| `source-status` | `hal:live` |

#### Gallica (BnF Digitized Collections)

| Field | Content |
|-------|---------|
| Protocol | SRU + REST (gallica.bnf.fr) |
| Lane | Explorer evidence |
| Geo | No pin (place mentions in text only) |
| Time | `date` field — year/decade precision |
| Maturity | full-text via `texteBrut` |
| `source-status` | `gallica:live` |

#### BnF Catalogue

| Field | Content |
|-------|---------|
| Protocol | SRU (catalogue.bnf.fr) |
| Lane | Explorer evidence |
| Geo | No pin (publication place text) |
| Time | `date` — year precision |
| Maturity | metadata-only |
| `source-status` | `bnf:live` |

#### Persée (French Academic Journals)

| Field | Content |
|-------|---------|
| Protocol | OAI-PMH + portal scrape |
| Lane | Explorer evidence |
| Geo | No pin |
| Time | `date` — year precision |
| Maturity | extraction_ready |
| `source-status` | `persee:live` |

#### theses.fr (French Theses)

| Field | Content |
|-------|---------|
| Protocol | REST API (theses.fr) |
| Lane | Explorer evidence |
| Geo | No pin (institution text) |
| Time | `dateSoutenance` — day precision when defended |
| Maturity | metadata-only |
| `source-status` | `theses_fr:live` |

#### OpenAlex (Scholarly Works)

| Field | Content |
|-------|---------|
| Protocol | REST API (api.openalex.org) |
| Lane | Explorer evidence |
| Geo | No pin (institution ROR, no coords) |
| Time | `publication_year` — year precision |
| Maturity | metadata-only |
| `source-status` | `openalex:live` |

---

### Vague B: Grounding & Structured Life Facts

#### Getty TGN (Place Identity)

| Field | Content |
|-------|---------|
| Protocol | SPARQL (vocab.getty.edu/sparql) |
| Lane | Grounding only — **not explorer evidence** |
| Geo | Returns TGN ID + sameAs Wikidata QID; coords via P625 later |
| Time | N/A (place identity, not events) |
| Maturity | live (PlaceIdentityResolver) |
| `source-status` | N/A (not a SourceConnector) |

#### WHG (World Historical Gazetteer)

| Field | Content |
|-------|---------|
| Protocol | REST API (whgazetteer.org/api) |
| Lane | Grounding only — **not explorer evidence** |
| Geo | Returns WHG ID + linked Wikidata QID; coords via P625 later |
| Time | Historical attestations with temporal scope |
| Maturity | live when `WHG_API_TOKEN` set (PlaceIdentityResolver) |
| `source-status` | N/A (not a SourceConnector) |

#### FranceArchives (Structured Life Facts)

| Field | Content |
|-------|---------|
| Protocol | SPARQL (francearchives.gouv.fr/sparql) |
| Lane | Explorer evidence (structured facts → candidates → gates) |
| Geo | Place names in EAC-CPF; resolve via grounding chain |
| Time | Birth/death dates — day/year precision |
| Maturity | extraction_ready |
| `source-status` | `france_archives:live` |

#### Deutsche Biographie + GND (German Biographical Data)

| Field | Content |
|-------|---------|
| Protocol | REST API (lobid.org/gnd) |
| Lane | Explorer evidence (structured facts → candidates → gates) |
| Geo | Place of birth/death as text; resolve via grounding chain |
| Time | `dateOfBirth`, `dateOfDeath` — day/year precision |
| Maturity | extraction_ready |
| `source-status` | `deutsche_biographie:live` |

#### POP/Mérimée (French Heritage Monuments)

| Field | Content |
|-------|---------|
| Protocol | REST API (api.pop.culture.gouv.fr) |
| Lane | Explorer evidence (place entity with coords) |
| Geo | WGS84 coords in `POP_COORDONNEES` — **can contribute to map_eligible** |
| Time | Construction/modification dates — year precision |
| Maturity | extraction_ready |
| `source-status` | `pop_merimee:live` |

---

### Deferred / Agora-Only Sources

| Source | Status | Notes |
|--------|--------|-------|
| ISIDORE | Deferred | French research discovery — Agora lane |
| OpenEdition | Deferred | French academic publisher — Agora lane |
| Chronicling America | Deferred | Historical newspaper OCR — Agora lane |
| VIAF | Stub | Identity joins only, no events |
| ISNI | Stub | Identity joins only, no events |
| IdRef | Stub | Identity joins only, no events |

---

## Gallica / text sources

Prefer `texteBrut` (or equivalent plain text) before ALTO when extracting quotes for evidence.

## Authority IDs worth wiring later (not inventing pins)

P268 (BnF), P214 (VIAF), P213 (ISNI), P269 (IdRef) — identity / join keys, not coordinates.

## Twin UX note

Progressive explorer UX contracts live in `docs/PROGRESSIVE_UX.md` (PR #19). Keep the same Explorer ≠ Agora rule on both PRs.

## Out of scope unless explicitly briefed

- Lowering lifespan / attribution gates to get "more points".
- Inventing coordinates without a traced `coord` source.
- Promising catalog density on the map before explorer wiring exists.
- Auto-creating pins from catalog search hits.

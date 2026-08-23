# Person dump → AI → Explorer / Agora

Date: 2026-08-23  
Status: approved — implementation in progress (2026-08-23)  
Supersedes (product path): dual `legacy` / `quality` ingest as the way to fill the map.  
Does not delete existing tables, dumps, or academic connectors.

## One sentence

For each person: dump as much **raw sourced text** as we can, let the **LLM read it**, emit **verified map facts** (Explorer) and **verified debates** (Agora). Databases stay the proof. The model does not invent.

## Problem

The Explorer mixed regex extractors, two pipelines, and linked biographies. Result: Schrödinger’s Dublin wedding on Marie Curie; Verdun as a Curie battle; density over correctness.

What we actually want:

- **Explorer** — maximum **true** geolocated facts (major/minor, direct/indirect) that draw a life trace **and** material traces (statue, plaque, tomb, house).
- **Agora** — theories, theses, controversies, attributions, origin debates from the same dumps. Not map pins.

## Non-goals

- A third silent pipeline beside quality/legacy that the UI must choose.
- LLM-invented coordinates or unsourced life events.
- Auto-promoting `pipeline='legacy'` rows into the new facts.
- Hitting a density floor (500 points). Density is a by-product of sources.
- Using the model as a historian without a stored quote.

## Product surfaces

| Surface | Unit | Examples | Not |
|---------|------|----------|-----|
| Explorer map / timeline | One **fact** with place and/or date + citation | Birth Warsaw; radium isolation Paris; WWI X-ray cars; statue in a square; treaty they signed | A sentence about someone else; a debate about birthplace |
| Agora | One **proposition** with authors/sources | “Was this text by X?”; origin theses; historiographic controversy | A new birth pin because a thesis disagrees |

A Catalan/Jewish/Swiss origin thesis is Agora. Birth stays one sourced fact unless a **sourced correction** supersedes it (append-only, existing supersession).

## Single ingest per person

```text
resolve person (Wikipedia title + Wikidata QID)
        │
        ▼
DUMP BRUT  →  raw_documents / corpus_documents / wiki pages (unchanged stores)
        │
        ▼
AI READ   →  grounded JSON (quote must appear in stored text)
        │
        ├── facts     → canonical_events (one pipeline label: `person`)
        └── debates   → claims (exportable=true, Intuition/Agora)
        │
        ▼
gazetteer / Wikidata P625  →  map_eligible iff real coords
```

Search in the Explorer **starts this ingest** if the person has no `person` run yet (or user re-runs). One job, two writers.

### Dump order (same job, two waves)

**Wave 1 — life trace (blocking for first map paint)**  
Wikipedia biography (user language + en/fr if missing) + Wikidata statements (P19, P20, P551, P937, P69, P26, battles they **participated in**, etc.). Follow only: places, works, institutions, battles **with person as participant**. Never ingest another person’s biography as if it were the subject.

**Wave 2 — academic / Agora (same job, map already usable)**  
Existing connectors: OpenAlex, theses.fr, HAL, Persée, Gallica, BnF, Internet Archive, Europeana when keyed. Store notices + abstracts (no PDF bytes in v1). AI reads titles/abstracts for Agora; if an abstract states a dated place about **this** person, it may also emit an Explorer fact with that quote.

## Grounded extraction (LLM)

Reuse `OPENAI_API_KEY` / `OPENAI_MODEL` (Talaria v1 project key). The current client (`llm.rs`) only pings and **must not** stay display-only.

For each chunk of stored text the model returns zero or more items:

```json
{
  "lane": "fact" | "debate",
  "event_type": "birth|death|residence|travel|battle|treaty|diplomatic|office|education|work|anecdote|commemoration|other",
  "role": "direct" | "indirect",
  "time": { "year": 1914, "precision": "year" },
  "place_surface": "Paris",
  "summary": "…",
  "quoted_text": "exact substring of the chunk",
  "document_id": "uuid",
  "confidence": 0.0
}
```

**Hard rules**

1. `quoted_text` must be a substring of the stored document (code-checked, not trusted).
2. Subject of the fact is the ingest person. If the quote’s grammatical agent is another named person, **drop** (`lane=fact`).
3. `lane=debate` never creates `map_eligible` events.
4. Coordinates never come from the model. Gazetteer / Wikidata only. No place → timeline only.
5. Commemorations (`statue`, `plaque`, `museum naming`) are valid **facts** if the quote locates the object. They are typed `commemoration`, not `residence`.
6. Partial dates stay typed (`year` / `month`). Do not coerce year → 1 Jan.

Batch chunks (~2–4k chars). Fail open: if the key is missing, ingest still stores dumps; Explorer shows Wikidata-structured facts only (infobox/P-statements), no regex biography harvest.

## Persistence

Keep all current stores. New writes:

| Table | Role |
|-------|------|
| `raw_documents` / `document_snapshots` / wiki pages | Brute dump, immutable |
| `canonical_events` | Explorer facts, `pipeline='person'`, `is_active`, evidence quote |
| `event_evidence` | `quoted_text` + document pointer |
| `claims` | Agora debates, `exportable=true` |
| `corpus_documents` | Academic notices (unchanged) |

API default for timeline/geojson: `pipeline=person` and `is_active=true`.  
`legacy` / `quality` remain in DB for audit; not shown unless `pipeline=` is explicit.

Occurrence identity: existing `occurrence_key` (subject + type + role + time + place). Extra sources **reinforce** (`source_count++`), they do not duplicate pins.

## Explorer vs Agora UI

- Explorer map/timeline: `person` facts only. Types include life events **and** commemorations. Filters by type allowed later; v1 shows all map-eligible facts.
- Event detail: citation first (quote + open source). No Schrödinger paragraph matched on year+place alone.
- Agora: claims from this ingest (theories, controversies, attribution). Linked to documents, never to a fake pin.

## Error handling

- OpenAI timeout / 429: retry with backoff; mark run `partial`; keep already accepted facts.
- Quote mismatch: drop the item, log, do not store.
- Geocode miss: keep timeline fact, `map_eligible=false`.
- Dump fetch fail for one source: skip source, continue others.

## Tests

- Quote must be substring or reject.
- Curie + Schrödinger marriage paragraph → 0 Curie facts.
- Curie statue in Warsaw quote → 1 `commemoration` fact, map-eligible if geocoded.
- Origin controversy abstract → 1 claim, 0 extra birth pins.
- API geojson default excludes `legacy`/`quality` unless requested.
- Fixture ingest (no network): Wikipedia fixture file + mocked LLM JSON.

## Rollout

1. Wire grounded LLM writer + `pipeline=person` + API default.
2. Explorer ingest = this job (wave 1 then 2).
3. Stop using regex density extractors on foreign bios.
4. Leave old rows in place; do not migrate them into `person`.

## Open follow-ups (out of v1)

- PDF full text
- User toggle “life only” vs “life + commemorations”
- On-chain Intuition publish (claims already `exportable`)

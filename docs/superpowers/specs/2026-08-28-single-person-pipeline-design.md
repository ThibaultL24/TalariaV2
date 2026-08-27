# Single `person` pipeline

Date: 2026-08-28  
Status: revised — 2026-08-28 (not implementation-ready until this revision is approved)  
Decisions: clean slate of **canonical** events via an explicit rebuild command (not a schema migration); LLM extracts prose, rules extract structured sources; failed and reviewable candidates remain in `event_candidates` — only accepted candidates materialize a `canonical_event`; unified lane keeps the name `person`; for a Wikimedia-resolved entity, the QID is the canonical **external alignment key**, the Talaria UUID remains the internal identity.

## Goal

Collapse three coexisting lanes (`legacy`, `quality`, `person`) into **one** pipeline named `person`, keeping what each one did best.

`quality` yields more events (3037 active vs 448) but a majority is noise. `person` yields fewer events with a far better map ratio (410/448 = 91 % vs 1127/3037 = 37 %) but has no biographical gates at all. The unified lane takes `person`'s Wikimedia-first collection and LLM extraction, and `quality`'s deterministic gates plus its BFS title crawl — which is where the density actually comes from.

The density must no longer be won by uncontrolled attribution.

### Evidence driving this design

Measured on the development database, 2026-08-28:

| Symptom | Lane | Cause |
|---|---|---|
| 1843 `battle` of 3037 events (61 %), e.g. `Victor Hugo — battle (1884)`, `Charles de Gaulle — battle (1943) @ Prijepolje` | quality | crawled battle pages attributed to the subject with no attribution check |
| `Louis XVI — anecdote (1981)`, `Louis XVI — work (1987) @ Paris` | person | `apply_gates` is never called; no birth/death bounds |
| `Christopher Columbus — arrival (1453)` | quality | `GateContext` had no birth year |
| `start_time` = `YYYY-06-15` vs `YYYY-01-01` | quality vs person | two different year-coercion conventions; `start_time` treated as semantics |
| `Marie curie — other (1895) @ ` with `confidence 0.98` | person | confidence is a constant, not a measure |
| `travel @ "the United States"`, `travel @ "Assemblée"` | person | destructive place normalisation; `InvalidPlaceKind` gate not wired |
| 3 entities on `Q7186`, 2 on `Q517`, `napoleon` with no QID, 14 `Napoleon LotD <uuid>` | both | entity row created before QID resolution |
| 448 person events invisible in the UI | frontend | client hardcodes `pipeline=quality`, SQL filters on equality |

## Non-goals

- Recomputing `confidence`. The field may remain on the row but is **not exposed** on timeline/geojson until it is an explained formula.
- Removing the `pipeline` column. It stays, with a single legal value `'person'`. Explicit `?pipeline=quality` or `?pipeline=legacy` returns a documented error (not silent empty).
- A no-LLM fallback that reconstructs prose density. Without `OPENAI_API_KEY` the unified lane produces only structured facts.
- Dropping `event_candidates`. That table is the quarantine. Fine-grained provenance does **not** collapse to document-level `raw_documents` alone.
- The Agora. `soft_claims` is untouched.
- Any subject-specific rule. No Napoleon hardcoding in gates.
- Putting a `TRUNCATE` of live event data inside a schema migration.

## Architecture — one pipeline, modular entry

One **pipeline** named `person`, one **entry point**, not one 3000-line file.

```text
crates/talaria-api/src/person_ingest/
  mod.rs          // orchestration only
  resolve.rs      // QID before entity write
  collect.rs      // Wikimedia fetch + BFS crawl
  extract.rs      // structured rules vs LLM prose
  grounding.rs    // two modes: text span vs structured pointer
  typing.rs       // TypedTime + place resolution
  gating.rs       // apply_gates + SubjectAttribution + role-aware lifespan
  persist.rs      // candidates always; canonical only on Accept
```

Steps:

1. **Resolve** subject → QID **before** any entity write. The Talaria UUID is the internal identity; the QID is the external alignment key when Wikimedia-resolved.
2. **Collect** from Wikimedia APIs: Wikipedia REST extract (multi-language), Wikidata statements, WDQS participation events, then BFS title crawl ported from `lot_e.rs`. Every fetch lands in `raw_documents` **and** a snapshot/locator sufficient for later evidence (revision id, section, offsets or statement id).
3. **Extract** by source nature: structured (Wikidata, WDQS, infobox, chronological lists) → rule extractors; prose → `llm::extract_chunk`. Both emit the same `RawExtractItem`. Extractor kind, model, prompt id and schema version are recorded on the **run**, not guessed later.
4. **Ground** with two modes (verbatim quote is **not** the structured path):
   - prose → `text_span` (quote + character offsets + section);
   - structure → `wikidata_statement` / JSON Pointer / row id (qid, pid, statement GUID, revision).
5. **Type**: `TypedTime` keeps **kind** and **precision** as two dimensions; place **surface is preserved**; search keys are generated; resolution writes `place_entity_id`.
6. **Gate** with `apply_gates`, including role-aware lifespan and `SubjectAttribution`.
7. **Persist**: every gated item is an `event_candidates` row. **Only `Accept` materializes a `canonical_event`.** An existing occurrence receives an idempotent `event_evidence` insert (`ON CONFLICT DO NOTHING`). Canonical events stay append-only.

### Two passes are mandatory

`GateContext` needs `subject_birth_year` and `subject_death_year` to bound **participatory** events. Birth and death must be established from Wikidata — the authoritative source — **before** the rest is judged.

- **Pass 1**: subject QID, birth, death, from Wikidata statements only. Populates `GateContext`.
- **Pass 2**: everything else, gated against that context.

This is the change that rejects `Louis XVI — anecdote (1981)` when the role implies presence.

## Quality contract

### Candidates vs canonical events (blocking)

A rejected or reviewable item is **not** a canonical event. Storing quarantine in `canonical_events` leaks through forgotten filters, exports, stats, and future APIs.

`event_candidates` is kept and is the quarantine:

| Gate decision | `event_candidates` | `canonical_events` |
|---|---|---|
| `Accept` | status `accepted`, `canonical_event_id` set | **new row** (or existing occurrence if `occurrence_key` matches) |
| `NeedsReview` | status `needs_review`, `rejection_codes` may be empty, review reason stored | **no row** |
| `Reject` | status `rejected`, `rejection_codes` non-empty | **no row** |

Density drop is explained by querying `event_candidates.rejection_codes`, not by scanning inactive canonical rows.

Timeline and geojson query **only** `canonical_events` with `is_active`. There is no `AND timeline_eligible` workaround required to hide rejects, because rejects never land there. `timeline_eligible` / `map_eligible` remain for **accepted** events that lack coords or have typed-but-partial time — not for quarantine.

`apply_gates` already covers nine rejection codes: `cross_clause_join`, `invalid_place_kind`, `event_before_subject_birth`, `event_after_subject_death`, `implausible_age_for_event_type`, `singleton_cardinality_violation`, `missing_evidence`, `duplicate_candidate`, `competing_place`.

### Lifespan gates are role-aware, not universal

The rule "no event outside the subject's lifespan is timeline-eligible" is too broad. It would drop legitimate posthumous facts: posthumous publication, transfer of remains, canonisation, monument inauguration, posthumous trial or rehabilitation, manuscript discovery, posthumous award.

Gates work on **role semantics**, not only `event_type`:

```text
If participant_role implies presence or action of the subject:
    apply birth/death bounds
Else if the event is merely about the subject:
    allow after death according to event type
    (publication, commemoration, burial transfer, award, …)
```

`event_after_subject_death` therefore applies to participatory roles (fought, travelled, resided, married, held office as a living actor). It does **not** apply to `commemoration` / `publication` when the evidence is posthumous *about* the subject.

### `SubjectAttribution` (v1)

New rejection code `subject_not_attributed`. This addresses the bulk of the 1843 false `battle` rows.

Attribution is classified, not a boolean:

| Match | Auto-accept on followed pages? |
|---|---|
| `direct_name_match` | yes, if role is **evidence-supported** |
| `alias_match` | yes, if role is evidence-supported |
| `title_subject_match` | yes (page is about the subject) |
| `structured_participant_match` | yes (WDQS / Wikidata statement names the subject) |
| `coreference_match` ("He then returned to Paris") | **NeedsReview**, never auto-accept in v1 |
| `unattributed` | Reject `subject_not_attributed` |

Facts from the subject's own biography page use `title_subject_match` and skip the followed-page name check.

An LLM-emitted role is **not** enough. The role must be supported by the grounded evidence (verb / structured property). A defaulted role on a followed battle page is `unattributed`.

v1 keeps high precision. Coreference is deferred to review, not invented.

## Append-only persist (blocking)

Canonical events are append-only: never mutated, only superseded.

**Forbidden:** `source_count++` on an existing row. Re-ingesting the same document would inflate the counter; concurrent ingest would drift; the row would be mutated.

**Required:** an evidence relation unique on content:

```text
event_evidence
  event_id
  raw_document_id
  source_locator      -- section / statement id / JSON pointer
  evidence_hash       -- hash of (locator + quote-or-statement-id)
  quote               -- nullable for structured evidence
  extractor_kind
  UNIQUE (event_id, raw_document_id, evidence_hash)
```

Persist:

1. Look up active canonical event by `occurrence_key` (subject + type + role + typed time + place entity + primary object).
2. If none: **insert** a new canonical event.
3. Insert evidence with `ON CONFLICT DO NOTHING`.
4. `source_count` / `evidence_count`, if kept on the event, are **derived** (`COUNT` of distinct evidence rows) and may be refreshed as a projection — they are not the source of truth. Prefer computing them at read time, or updating via a trigger/function that is a projection of evidence, never a blind increment.

Two distinct sources on the same occurrence → two evidence rows, one canonical event.  
The same source ingested twice → second evidence insert is a no-op.  
Two events the same day at the same place with different roles or objects → two occurrence keys, two events.

Competing places on one occurrence: existing `competing_place` gate; do not mutate the accepted place. A later better resolution uses **supersession**, not in-place edit.

## Typed time (blocking)

Do **not** collapse `kind` and `precision`. The existing `TypedTime` in `talaria-quality` already separates nature (`Exact` / `Range` / `Approx` / `Unknown`) from calendar fields. The stored JSON contract is:

```json
{
  "kind": "exact",
  "start": "1805",
  "end": null,
  "precision": "year",
  "calendar": "gregorian",
  "surface": "1805"
}
```

```json
{
  "kind": "approx",
  "start": "1805",
  "end": null,
  "precision": "year",
  "calendar": "gregorian",
  "surface": "vers 1805"
}
```

```json
{
  "kind": "range",
  "start": "1805-03",
  "end": "1805-06",
  "precision": "month",
  "calendar": "gregorian",
  "surface": "de mars à juin 1805"
}
```

`kind` ∈ `exact | range | approx | unknown`.  
`precision` ∈ `day | month | year` (absent when `kind=unknown`).

`start_time` remains a **derived SQL projection for index and order only** (year precision → 1 January of that year; month precision → first day of that month). It is never the semantic date. The API exposes `time_json` (or `kind`, `precision`, `surface`, `start`, `end`) and the frontend renders from that, never `start_time.slice(0, 4)`.

Implementation note: `person_ingest` currently writes `{ "kind": "year", "year": … }`. That shape is **wrong** and must stop. Serialise through the shared `time_to_json` / `TypedTime` path.

## Provenance

Document-level `raw_documents` is necessary but not sufficient. Each accepted or quarantined item must answer:

- which sentence / section / block, or which Wikidata property and statement GUID;
- which page revision;
- which extractor;
- which prompt and schema version (LLM path).

Prose evidence:

```json
{
  "kind": "text_span",
  "quote": "Napoleon arrived in Paris...",
  "start_offset": 1420,
  "end_offset": 1450,
  "section": "Return from Elba",
  "revision_id": 123456
}
```

Structured evidence:

```json
{
  "kind": "wikidata_statement",
  "qid": "Q517",
  "pid": "P39",
  "statement_id": "Q517$...",
  "revision_id": 2387456
}
```

`document_snapshots` / `document_fragments` may still back Wikipedia locators; they are not truncated as "orphans". The quality *orchestration* (`lot_e.rs` as the live explorer path, `quality.rs` fixture CLI) is what we retire, not the ability to point at a fragment.

## Entity alignment

For a Wikimedia-resolved entity, the QID is the canonical **external alignment key**. Internal identity remains the Talaria UUID. That leaves room for people absent from Wikidata, other registries, and later merges.

- Unique index `UNIQUE (qid) WHERE qid IS NOT NULL` — one QID, one Talaria entity, regardless of `kind`.
- QID resolution moves **before** entity creation.
- The user-typed surface form goes to `entity_aliases`.
- Existing duplicates merge on QID: keep the row whose name matches the Wikidata label; other names become aliases; **remap all foreign keys** onto the survivor.
- The 14 `Napoleon LotD <uuid>` rows are deleted as part of the **rebuild command**, not the schema migration — test artefacts with no QID.
- **Behaviour change**: an unresolvable QID makes Wikimedia ingest fail instead of creating a ghost entity. A later non-Wikimedia path can mint a UUID-only entity; it is out of v1 scope.

## Places

Do not mutate `place_surface` by stripping articles. `The Hague` must not become `Hague`.

1. Keep `place_surface` as written.
2. Generate search keys (with/without leading article, language variants).
3. Resolve to a place entity (gazetteer / Wikidata P625 / alias).
4. Display the **resolved entity label**.
5. `map_eligible` only when that entity has coordinates.
6. Wire `InvalidPlaceKind`: `place_entity.kind` must be `place` (`@ Assemblée` fails).

```json
{
  "surface": "the United States",
  "normalized_query": "United States",
  "place_entity_id": "...",
  "resolution_method": "wikidata_alias"
}
```

## Schema migration vs rebuild

### Migration `027_unified_person_pipeline.sql` — structure only

- `CHECK (pipeline = 'person')` with `DEFAULT 'person'`.
- Rebuild partial indexes currently scoped `WHERE pipeline = 'quality'` onto `'person'`.
- Drop `uq_canonical_events_active_person_occurrence` (duplicates `uq_canonical_active_occurrence`).
- Unique `entities(qid) WHERE qid IS NOT NULL`.
- Evidence uniqueness `UNIQUE (event_id, raw_document_id, evidence_hash)` (add `evidence_hash` / `source_locator` if missing).
- **No** `TRUNCATE`. **No** `rejection_codes` on `canonical_events` (codes live on `event_candidates`).

Migrations are embedded at compile time; rebuild `talaria-store` before `talaria migrate`.

### Command `talaria admin rebuild-person-pipeline`

Destructive rebuild is an explicit operator action:

```bash
talaria admin rebuild-person-pipeline \
  --confirm-destruction \
  --backup-manifest ./rebuild-manifest.json
```

1. Count rows in scope (`canonical_events`, `event_evidence`, `event_candidates`, LotD entities).
2. Write a manifest / snapshot of counts and sample ids.
3. Purge only that scope (`canonical_events` CASCADE evidence; rebuild candidates; merge QID duplicates with FK remap).
4. Re-ingest is **not** implicit: the command purges and verifies empty canonical tables, then the operator runs ingest. Alternatively the command may take `--reingest-subjects file` as an explicit second phase.
5. Verify invariants (pipeline check, unique QID, no canonical row with non-empty rejection semantics).
6. Fail if canonical count is unexpectedly zero after a requested reingest, or if FK remap left orphans.

Dev purge is defensible. Hiding it in a migration is not.

### Governance

`AGENTS.md` currently mandates "Legacy vs quality coexistence is strict". Those rules become false and must be rewritten in the same change.

## API and frontend

| Surface | Change |
|---|---|
| `resolve_pipeline` | default `'person'` |
| `?pipeline=quality` / `legacy` | **400** with `{ "error": "pipeline_retired", "use": "person" }` — not an empty list |
| omitted `pipeline` | same as `'person'` |
| timeline / geojson | `canonical_events` only; drop `confidence`; add `time` (`kind`, `precision`, `surface`, `start`, `end`) |
| `timelineSearchParams` | default `'person'` |
| `mapTimelineEventToItem` | render from `time`, never slice `start_time` |

### Code removed vs kept

Removed as **live explorer orchestration**: `lot_e.rs` density ingest, quality orchestration in `ingest.rs`, `quality.rs` fixture CLI as the explorer path.

Kept: `talaria-quality` (gates, `TypedTime`, occurrence keys), `event_candidates`, snapshot/fragment tables as locators, `talaria-sources` connectors for Agora and structured extractors.

## Testing

Regressions remain anchored on measured DB cases, plus the invariants this revision adds.

| Case | Expected |
|---|---|
| `Louis XVI — anecdote (1981)` with participatory role | candidate `rejected` `event_after_subject_death`; **no** canonical row |
| `Victor Hugo — battle (1884)` from a followed battle page | `unattributed` → `subject_not_attributed`; no canonical row |
| `Christopher Columbus — arrival (1453)` | `event_before_subject_birth`; no canonical row |
| Posthumous publication / commemoration after death | candidate accepted (about-subject role); canonical row |
| `Marie Curie`, 3 rows on `Q7186` | merged to 1; FKs remapped |
| `travel @ "the United States"` | surface preserved; resolved entity label `United States` |
| `The Hague` | surface preserved; must **not** resolve as `Hague` |
| `travel @ "Assemblée"` | `invalid_place_kind`; no canonical row |
| Same corpus ingested twice | evidence counts unchanged (idempotent) |
| Two distinct sources, same occurrence | one canonical event, two evidence rows |
| Same day, same place, different role or object | two canonical events |
| Competing places | `competing_place`; no in-place mutation |
| `kind=approx` / `range` / month-only / year-only | stored in `time_json`; `start_time` is projection only |
| Missing or shifted quote | grounding fail; no accept |
| Wikidata statement id grounding | structured evidence, no verbatim quote required |
| Multilingual alias | `alias_match` |
| Coreference-only sentence on a followed page | `NeedsReview`, not auto-accept |
| API without `pipeline` | person events |
| `pipeline=quality` | explicit error |
| Concurrent ingest of the same occurrence | one canonical row, no duplicate key failure |
| BFS budget, cycles, forbidden namespaces | bounded, no infinite crawl |
| LLM model / prompt / schema | recorded on the run |

`cargo test -p talaria-quality` for gates; one end-to-end fixture test of the unified path over recorded Wikipedia and Wikidata payloads, no network.

## Success criteria

- One legal value in `canonical_events.pipeline`.
- Search-bar ingest produces events visible on map and timeline.
- No **participatory** event outside the subject's lifespan becomes a canonical event.
- Posthumous *about-subject* types can still become canonical events.
- `battle` is no longer the majority event type for non-military subjects.
- Every rejected or reviewable item exists in `event_candidates` with codes/status; **zero** such rows in `canonical_events`.
- Re-ingesting the same corpus does not change evidence cardinality.
- `time_json.kind` is never `day|month|year`.
- `cargo test` and `cargo clippy --all-targets --all-features` pass; `npm run build` typechecks.

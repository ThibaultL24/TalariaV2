# Single `person` pipeline

Date: 2026-08-28  
Status: approved — 2026-08-28  
Decisions: clean slate on `canonical_events`; LLM extracts prose, rules extract structured sources; failed gates land in typed quarantine; unified lane keeps the name `person`; entity identity is the QID.

## Goal

Collapse three coexisting lanes (`legacy`, `quality`, `person`) into **one** pipeline named `person`, keeping what each one did best.

`quality` yields more events (3037 active vs 448) but a majority is noise. `person` yields fewer events with a far better map ratio (410/448 = 91 % vs 1127/3037 = 37 %) but has no biographical gates at all. The unified lane takes `person`'s Wikimedia-first collection and LLM extraction, and `quality`'s deterministic gates plus its BFS title crawl — which is where the density actually comes from.

### Evidence driving this design

Measured on the development database, 2026-08-28:

| Symptom | Lane | Cause |
|---|---|---|
| 1843 `battle` of 3037 events (61 %), e.g. `Victor Hugo — battle (1884)`, `Charles de Gaulle — battle (1943) @ Prijepolje` | quality | crawled battle pages attributed to the subject with no attribution check |
| `Louis XVI — anecdote (1981)`, `Louis XVI — work (1987) @ Paris` | person | `apply_gates` is never called; no birth/death bounds |
| `Christopher Columbus — arrival (1453)` | quality | `GateContext` had no birth year |
| `start_time` = `YYYY-06-15` vs `YYYY-01-01` | quality vs person | two different year-coercion conventions |
| `Marie curie — other (1895) @ ` with `confidence 0.98` | person | confidence is a constant, not a measure |
| `travel @ "the United States"`, `travel @ "Assemblée"` | person | no place normalisation, `InvalidPlaceKind` gate not wired |
| 3 entities on `Q7186`, 2 on `Q517`, `napoleon` with no QID, 14 `Napoleon LotD <uuid>` | both | entity row created before QID resolution |
| 448 person events invisible in the UI | frontend | client hardcodes `pipeline=quality`, SQL filters on equality |

## Non-goals

- Recomputing `confidence`. The field keeps its current values but is **no longer exposed** to the frontend until it means something.
- Removing the `pipeline` column, parameter, or client argument. It stays, with a single legal value.
- A no-LLM fallback mode. Without `OPENAI_API_KEY` the unified lane produces only structured facts.
- Keeping the provenance layer (`document_snapshots` → `document_fragments` → `event_candidates`). Provenance stays at document level in `raw_documents`.
- The Agora. `soft_claims` is untouched and survives the purge (FK is `SET NULL`).
- Any subject-specific rule. No Napoleon hardcoding in gates.

## Architecture — one path, seven steps

Single module: `crates/talaria-api/src/person_ingest.rs`.

1. **Resolve subject → QID** before any entity write.
2. **Collect** from Wikimedia APIs only: Wikipedia REST extract (multi-language), Wikidata statements, WDQS participation events, then BFS title crawl ported from `lot_e.rs`. Every fetched page lands in `raw_documents`.
3. **Extract** by source nature: structured (Wikidata, WDQS, infobox, chronological lists) → rule extractors; prose → `llm::extract_chunk`. Both emit `RawExtractItem`.
4. **Ground** with `accept_items`: the quote must exist verbatim in the document. Kills hallucinations.
5. **Type**: `TypedTime` preserves date precision; place is normalised and resolved.
6. **Gate** with `apply_gates`.
7. **Persist**: dedup on `occurrence_key`; an existing occurrence is reinforced (`source_count++`) rather than duplicated.

### Two passes are mandatory

`GateContext` needs `subject_birth_year` and `subject_death_year` to bound anything. Birth and death must therefore be established from Wikidata — the authoritative source — **before** the rest is judged.

- **Pass 1**: subject QID, birth, death, from Wikidata statements only. Populates `GateContext`.
- **Pass 2**: everything else, gated against that context.

This is the single change that rejects `Louis XVI — anecdote (1981)`.

## Quality contract

`apply_gates` already covers nine rejection codes: `cross_clause_join`, `invalid_place_kind`, `event_before_subject_birth`, `event_after_subject_death`, `implausible_age_for_event_type`, `singleton_cardinality_violation`, `missing_evidence`, `duplicate_candidate`, `competing_place`.

### One gate to write: `SubjectAttribution`

New rejection code `subject_not_attributed`. This is the gate that addresses 61 % of the current noise.

A fact extracted from a **followed** page — not the subject's own biography — is attributed to the subject only if both hold:

- the subject's name or a known alias appears in the quoted sentence, and
- the extractor assigned an explicit role (`item.role`) rather than a default.

Otherwise the candidate is quarantined. Facts extracted from the subject's own biography page skip this gate.

### Gate decision → columns

| Decision | `historically_valid` | `timeline_eligible` | `map_eligible` | `epistemic_status` |
|---|---|---|---|---|
| `Accept` | true | true | true when coordinates resolved | `attested` |
| `NeedsReview` | true | false | false | `uncertain` |
| `Reject(codes)` | false | false | false | `uncertain` |

Quarantined rows are written, never displayed. They answer "why did density drop" without guesswork.

## Entity canonicalisation

The QID is the identity.

- Unique index on `entities(qid)` where `kind='person'` and `qid IS NOT NULL`.
- QID resolution moves **before** entity creation. Today `open_db_for_subject` creates the row and `resolve_person_qid` runs after, which is precisely what produced `napoleon` / `Napoleon` / `Napoleon Bonaparte`.
- The user-typed surface form goes to `entity_aliases`.
- Existing duplicates merge on QID: keep the row whose name matches the Wikidata label, demote other names to aliases.
- The 14 `Napoleon LotD <uuid>` rows are deleted — test artefacts with no QID.
- **Behaviour change**: an unresolvable QID makes ingestion fail instead of creating a ghost entity.

## Typed dates and places

`time_json` is the source of truth, with `kind` in `day` | `month` | `year`.

`start_time` stays populated but is **ordering and SQL filtering only**, with one convention instead of the current two: year precision resolves to 1 January, month precision to the first day of that month. The API exposes `time_precision` and `time_surface`; the frontend renders from those instead of `start_time.slice(0, 4)`.

Places: trim, strip leading articles (`the United States` → `United States`). A place must resolve to coordinates via the offline gazetteer or Wikidata P625 to earn `map_eligible`; otherwise the event stays timeline-only. The existing `InvalidPlaceKind` gate gets wired by resolving the place to an entity and checking `kind='place'`, which rejects `@ Assemblée`.

## Migration `027_unified_person_pipeline.sql`

1. `TRUNCATE canonical_events CASCADE` — takes `event_evidence` with it. `soft_claims`, `quality_claims`, and `event_candidates` are `SET NULL`, so the Agora survives.
2. Truncate the orphaned provenance tables `document_snapshots`, `document_fragments`, `event_candidates`. Their schema stays — dropping the tables would mean editing migrations 006 and 008, which is out of scope.
3. `CHECK (pipeline = 'person')` with `DEFAULT 'person'`.
4. Rebuild on `'person'` the seven partial indexes currently scoped `WHERE pipeline = 'quality'`: `idx_canonical_events_map_eligible_quality`, `idx_canonical_events_occurrence_stem`, `idx_canonical_events_subject_type_time`, `idx_canonical_events_timeline_eligible`, `uq_canonical_active_occurrence`, `uq_canonical_active_singleton_birth_death`, plus `uq_canonical_events_active_fingerprint`.
5. Drop `uq_canonical_events_active_person_occurrence` — it duplicates `uq_canonical_active_occurrence`.
6. `ADD COLUMN rejection_codes jsonb NOT NULL DEFAULT '[]'`.
7. Entities: merge by QID, delete `LotD` artefacts, add the unique QID index.

Migrations are embedded at compile time by `talaria-store`; rebuild before running `talaria migrate`.

### Governance

`AGENTS.md` currently mandates "Legacy vs quality coexistence is strict" over roughly ten lines. Those rules become false. They must be rewritten in the same change, or a later session will restore the split this work removes.

## API and frontend

| Surface | Change |
|---|---|
| `resolve_pipeline` (`routes/events.rs:40`) | default `'quality'` → `'person'` |
| `list_timeline_events` (`canonical_events.rs:158`) | **add `AND ce.timeline_eligible`** — without it quarantine shows in the timeline |
| `list_geojson_events` | already filters `map_eligible`; unchanged |
| timeline payload | drop `confidence`, add `time_precision` and `time_surface` |
| `timelineSearchParams` (`web/src/lib/api.ts:146`) | default `'quality'` → `'person'` |
| `mapTimelineEventToItem` | render from `time_precision`, stop slicing `start_time` |

### Code removed

`lot_e.rs` (2662 lines), the quality orchestration in `ingest.rs`, and `quality.rs`. Kept: `talaria-quality` as the gate library, and `talaria-sources` for connectors and the Agora.

## Testing

Regressions are anchored on cases measured in the database, not invented ones.

| Case | Expected |
|---|---|
| `Louis XVI — anecdote (1981)` | rejected `event_after_subject_death` |
| `Victor Hugo — battle (1884)` | rejected `subject_not_attributed` |
| `Christopher Columbus — arrival (1453)` | rejected `event_before_subject_birth` |
| `Marie Curie`, 3 rows on `Q7186` | merged to 1 |
| `travel @ "the United States"` | normalised to `United States` |
| `travel @ "Assemblée"` | rejected `invalid_place_kind` |

Plus unit tests for `SubjectAttribution` in `cargo test -p talaria-quality`, and one end-to-end fixture test of the unified path over recorded Wikipedia and Wikidata payloads, no network.

## Success criteria

- One legal value in `canonical_events.pipeline`.
- Events produced by the search bar are visible on the map and timeline.
- No event outside its subject's lifespan is `timeline_eligible`.
- `battle` is no longer the majority event type for non-military subjects.
- Every quarantined row carries at least one code in `rejection_codes`.
- `cargo test` and `cargo clippy --all-targets --all-features` pass; `npm run build` typechecks.

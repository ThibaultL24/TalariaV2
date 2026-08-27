# Single Person Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One ingest pipeline named `person` that writes canonical events only after gates accept a candidate, with typed time, QID-first entity alignment, and idempotent evidence — so search-bar ingest is visible and lifespan/attribution noise never becomes a map point.

**Architecture:** Domain rules live in `talaria-quality` (time JSON, role-aware gates, attribution). Persistence lives in `talaria-store` (candidates as quarantine, canonical insert-only, unique evidence). Orchestration is `talaria-api/src/person_ingest/` (resolve → collect → extract → ground → type → gate → persist). HTTP default `pipeline=person`; retired lanes return 400. Destructive wipe is `talaria admin rebuild-person-pipeline`, never a `TRUNCATE` in a migration.

**Tech Stack:** Rust workspace (`talaria-quality`, `talaria-store`, `talaria-api`), Postgres/PostGIS, sqlx migrations embedded in `talaria-store`, Vite/React explorer (`web/src`).

## Global Constraints

- Failed and reviewable items stay in `event_candidates`; only `Accept` materializes a `canonical_event`.
- Canonical events are append-only; never `source_count++`; evidence uses `ON CONFLICT DO NOTHING`.
- `time_json.kind` ∈ `exact|range|approx|unknown`; `precision` ∈ `day|month|year`; `start_time` is a SQL projection only.
- QID is the external alignment key; Talaria UUID is internal identity. `UNIQUE (qid) WHERE qid IS NOT NULL`.
- Do not strip articles from `place_surface` before resolution (`The Hague` stays).
- Lifespan gates apply only when the role implies the subject’s presence; posthumous *about* types may accept after death.
- `?pipeline=quality` and `?pipeline=legacy` return 400 `pipeline_retired`, not an empty list.
- Tests: fixtures only; no live network in CI (`cargo test`).
- Do not put `TRUNCATE canonical_events` in `027_*.sql`.
- Do not delete `lot_e.rs` in this plan: CLI density/place reports still import it. HTTP explorer must stop calling quality/Lot E ingest.
- After adding `migrations/027_*.sql`, rebuild (`cargo build -p talaria-api`) before migrate — sqlx embeds at compile time.
- Git in this environment is 2.25: commit with `git commit -F /tmp/msg.txt` (no `--trailer`).
- No Napoleon-hardcoded years or title lists in gates.

## File map

| Path | Responsibility |
|---|---|
| `crates/talaria-quality/src/time_typed.rs` | `time_to_json` contract + `start_time_from_typed` projection |
| `crates/talaria-quality/src/gates.rs` | Role-aware lifespan + `SubjectAttribution` |
| `crates/talaria-quality/src/attribution.rs` | Attribution match classification |
| `crates/talaria-quality/src/places.rs` | Place search keys; surface preserved |
| `migrations/027_unified_person_pipeline.sql` | Constraints, indexes, nullable candidate locators, evidence hash — **no purge** |
| `crates/talaria-store/src/person_events.rs` | Idempotent evidence; stop `reinforce_person_event` increment |
| `crates/talaria-store/src/entities.rs` | QID-first upsert |
| `crates/talaria-api/src/person_ingest/` | Modular ingest entry |
| `crates/talaria-api/src/routes/events.rs` | Default pipeline, retired error, time payload |
| `web/src/lib/api.ts`, `web/src/features/events/mappers/timeline.ts` | Client default + render `time` |
| `crates/talaria-api/src/rebuild.rs` | `admin rebuild-person-pipeline` |
| `AGENTS.md` | Drop legacy/quality coexistence rules |

Do **not** create `docs/` files beyond this plan. Fold AGENTS.md into Task 12.

---

### Task 1: Typed `time_json` contract

**Files:**
- Modify: `crates/talaria-quality/src/time_typed.rs` (`time_to_json`, `start_time_from_typed`)
- Test: same file `#[cfg(test)]`

**Interfaces:**
- Consumes: `crate::model::TypedTime`
- Produces:

```rust
pub fn time_to_json(time: &TypedTime) -> serde_json::Value
pub fn start_time_from_typed(time: &TypedTime) -> Option<chrono::DateTime<chrono::Utc>>
```

`time_to_json` must emit:

- `kind`: `exact` | `range` | `approx` | `unknown` (never `year`/`month`/`day`)
- `start` / `end`: ISO-ish strings (`"1805"`, `"1805-03"`, `"1805-03-21"`) or JSON null
- `precision`: `year` | `month` | `day` omitted when `kind=unknown`
- `calendar`: `"gregorian"`
- `surface`: original surface or null

`start_time_from_typed`: year-only → 1 January; month-only → day 1 of that month; full day → that date. Negative years: keep `None` for `start_time` (Postgres timestamptz) unless existing BCE handling already works — do not invent BCE UTC.

- [ ] **Step 1: Write the failing tests** at the bottom of `time_typed.rs` (existing tests stay).

```rust
#[test]
fn time_to_json_exact_year_uses_kind_exact_not_year() {
    let t = TypedTime::Exact {
        year: 1805,
        month: None,
        day: None,
        surface: Some("1805".into()),
    };
    let v = time_to_json(&t);
    assert_eq!(v["kind"], "exact");
    assert_eq!(v["precision"], "year");
    assert_eq!(v["start"], "1805");
    assert!(v.get("end").unwrap().is_null() || v["end"] == serde_json::Value::Null);
    assert_eq!(v["calendar"], "gregorian");
    assert_eq!(v["surface"], "1805");
}

#[test]
fn time_to_json_never_uses_precision_as_kind() {
    let t = TypedTime::Exact {
        year: 1805,
        month: Some(3),
        day: None,
        surface: Some("March 1805".into()),
    };
    let v = time_to_json(&t);
    assert_eq!(v["kind"], "exact");
    assert_eq!(v["precision"], "month");
    assert_eq!(v["start"], "1805-03");
}

#[test]
fn start_time_year_projects_to_january_first_not_june() {
    let t = TypedTime::Exact {
        year: 1805,
        month: None,
        day: None,
        surface: Some("1805".into()),
    };
    let dt = start_time_from_typed(&t).expect("projection");
    assert_eq!(dt.format("%Y-%m-%d").to_string(), "1805-01-01");
}

#[test]
fn start_time_month_projects_to_day_one() {
    let t = TypedTime::Exact {
        year: 1805,
        month: Some(3),
        day: None,
        surface: Some("March 1805".into()),
    };
    let dt = start_time_from_typed(&t).expect("projection");
    assert_eq!(dt.format("%Y-%m-%d").to_string(), "1805-03-01");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p talaria-quality time_to_json_exact_year -- --nocapture`

Expected: FAIL — current `time_to_json` is `serde_json::to_value(time)` so `kind` is `exact` but there is no `precision`/`start`/`calendar`; `start_time_from_typed` uses month 6 / day 15.

- [ ] **Step 3: Implement `time_to_json` and `start_time_from_typed`**

Replace `time_to_json` so it does **not** `to_value` the enum. Build the object by match. Keep `TypedTime` Rust enum unchanged (`Exact`/`Range`/`Approx`/`Unknown`).

For `Range { start_year, end_year, surface }`: `kind=range`, `start="{start_year}"`, `end="{end_year}"`, `precision=year`.

For `Approx { year, surface }`: `kind=approx`, `start="{year}"`, `precision=year`.

For `Unknown`: `kind=unknown`, no `precision`.

Replace mid-year defaults in `start_time_from_typed` (`month.unwrap_or(6)` / `day.unwrap_or(15)`) with `unwrap_or(1)`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p talaria-quality --lib`

Expected: PASS (existing `parse_typed_time` tests still pass).

- [ ] **Step 5: Commit**

```bash
git add crates/talaria-quality/src/time_typed.rs
# commit -F with subject: fix(quality): emit kind/precision time_json contract
```

---

### Task 2: Role-aware lifespan gates

**Files:**
- Modify: `crates/talaria-quality/src/gates.rs`
- Test: `crates/talaria-quality/src/gates.rs` `mod tests`

**Interfaces:**
- Consumes: `EventCandidate.event_type`, `EventCandidate.predicate`
- Produces:

```rust
pub fn event_implies_subject_presence(event_type: &str, predicate: &str) -> bool
```

Return `false` for about-subject types: `publication`, `commemoration`, `award` (and predicates `commemorated_at`, `published`, `awarded`). Everything else that currently hits `event_after_subject_death` stays participatory (`battle`, `travel`, `office`, `anecdote`, `residence`, `marriage`, `death` as presence, …).

In `apply_gates`, wrap the block that pushes `EventAfterSubjectDeath` when `ey > dy` with `if event_implies_subject_presence(...)`.

Keep `EventBeforeSubjectBirth` for all types except `birth` itself (Columbus 1453 still rejects).

- [ ] **Step 1: Write failing tests** next to existing gate tests, using `base_candidate`:

```rust
#[test]
fn participatory_anecdote_after_death_is_rejected() {
    let c = base_candidate("anecdote", 1981);
    let ctx = GateContext {
        subject_birth_year: Some(1754),
        subject_death_year: Some(1793),
        ..Default::default()
    };
    let codes = apply_gates(&c, &ctx).codes();
    assert!(codes.contains(&"event_after_subject_death".into()));
}

#[test]
fn commemoration_after_death_is_not_lifespan_rejected() {
    let c = base_candidate("commemoration", 1840);
    let ctx = GateContext {
        subject_birth_year: Some(1754),
        subject_death_year: Some(1793),
        ..Default::default()
    };
    assert!(!apply_gates(&c, &ctx)
        .codes()
        .contains(&"event_after_subject_death".into()));
}

#[test]
fn arrival_before_birth_is_still_rejected() {
    let c = base_candidate("arrival", 1453);
    let ctx = GateContext {
        subject_birth_year: Some(1451),
        subject_death_year: Some(1506),
        ..Default::default()
    };
    assert!(apply_gates(&c, &ctx)
        .codes()
        .contains(&"event_before_subject_birth".into()));
}
```

- [ ] **Step 2: Run** `cargo test -p talaria-quality participatory_anecdote_after_death -- --nocapture`

Expected: `commemoration_after_death` FAIL (today every `ey > dy` rejects).

- [ ] **Step 3: Implement `event_implies_subject_presence` and gate the after-death check.** Re-export from `lib.rs`.

- [ ] **Step 4: Run** `cargo test -p talaria-quality --lib`

Expected: PASS.

- [ ] **Step 5: Commit** `fix(quality): apply death bounds only to participatory events`

---

### Task 3: `SubjectAttribution`

**Files:**
- Create: `crates/talaria-quality/src/attribution.rs`
- Modify: `crates/talaria-quality/src/lib.rs` (`mod attribution; pub use …`)
- Modify: `crates/talaria-quality/src/gates.rs` (`RejectionCode::SubjectNotAttributed`)
- Test: `crates/talaria-quality/src/attribution.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: subject aliases, quote, page title, whether page is followed, structured flag, evidence-supported role
- Produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributionMatch {
    DirectNameMatch,
    AliasMatch,
    TitleSubjectMatch,
    StructuredParticipantMatch,
    CoreferenceMatch,
    Unattributed,
}

pub struct AttributionInput<'a> {
    pub subject: &'a str,
    pub aliases: &'a [&'a str],
    pub quote: &'a str,
    pub page_title: &'a str,
    pub from_followed_page: bool,
    pub structured_source: bool,
    pub role_supported_by_evidence: bool,
}

pub fn classify_attribution(input: &AttributionInput<'_>) -> AttributionMatch

pub fn auto_accept_attribution(m: AttributionMatch) -> bool
// true for DirectNameMatch, AliasMatch, TitleSubjectMatch, StructuredParticipantMatch
// false for CoreferenceMatch and Unattributed
```

Matching is case-insensitive substring on quote for name/aliases (reuse `fold_latin_accents` from store **or** a local lowercase fold in quality — do not add a `talaria-store` dep to quality; copy a tiny fold or use `to_lowercase`).

`TitleSubjectMatch`: `from_followed_page == false` (biography page) **or** `page_title` equals subject (ignore case).

`StructuredParticipantMatch`: `structured_source == true`.

`CoreferenceMatch` (v1, conservative): quote has no name/alias but starts with `He ` / `She ` / `The emperor ` / `L'empereur ` after trim — only if `from_followed_page`.

`Unattributed`: default for followed pages.

Do **not** auto-accept if `role_supported_by_evidence == false` on a followed page even when the name matches — that is the Victor Hugo battle case (name may appear in a list; role was defaulted). Exception: structured sources.

- [ ] **Step 1: Write tests** in `attribution.rs`:

```rust
#[test]
fn followed_battle_page_without_role_evidence_is_unattributed() {
    let m = classify_attribution(&AttributionInput {
        subject: "Victor Hugo",
        aliases: &["Hugo"],
        quote: "The Battle of Plevna was fought in 1877.",
        page_title: "Siege of Plevna",
        from_followed_page: true,
        structured_source: false,
        role_supported_by_evidence: false,
    });
    assert_eq!(m, AttributionMatch::Unattributed);
    assert!(!auto_accept_attribution(m));
}

#[test]
fn biography_page_is_title_subject_match() {
    let m = classify_attribution(&AttributionInput {
        subject: "Victor Hugo",
        aliases: &[],
        quote: "He was born in Besançon.",
        page_title: "Victor Hugo",
        from_followed_page: false,
        structured_source: false,
        role_supported_by_evidence: true,
    });
    assert_eq!(m, AttributionMatch::TitleSubjectMatch);
    assert!(auto_accept_attribution(m));
}

#[test]
fn coreference_on_followed_page_is_not_auto_accept() {
    let m = classify_attribution(&AttributionInput {
        subject: "Napoleon",
        aliases: &[],
        quote: "He then returned to Paris.",
        page_title: "War of the Sixth Coalition",
        from_followed_page: true,
        structured_source: false,
        role_supported_by_evidence: true,
    });
    assert_eq!(m, AttributionMatch::CoreferenceMatch);
    assert!(!auto_accept_attribution(m));
}

#[test]
fn wdqs_structured_is_structured_participant() {
    let m = classify_attribution(&AttributionInput {
        subject: "Napoleon",
        aliases: &[],
        quote: "",
        page_title: "WDQS events for Q517",
        from_followed_page: true,
        structured_source: true,
        role_supported_by_evidence: true,
    });
    assert_eq!(m, AttributionMatch::StructuredParticipantMatch);
}
```

- [ ] **Step 2: Run** `cargo test -p talaria-quality classify_attribution -- --nocapture`

Expected: FAIL compile (`attribution` module missing).

- [ ] **Step 3: Implement module + add `SubjectNotAttributed` to `RejectionCode` (`as_str` = `subject_not_attributed`).** Do not call it from `apply_gates` yet — persist/gating in Task 8 maps `Unattributed` → that code. Optionally add a helper `attribution_gate_decision(m) -> GateDecision` that returns `NeedsReview` for coreference, `Reject([SubjectNotAttributed])` for unattributed, `Accept` otherwise.

- [ ] **Step 4: Run** `cargo test -p talaria-quality --lib`

Expected: PASS.

- [ ] **Step 5: Commit** `feat(quality): classify subject attribution on followed pages`

---

### Task 4: Place search keys (no destructive article strip)

**Files:**
- Create: `crates/talaria-quality/src/places.rs`
- Modify: `crates/talaria-quality/src/lib.rs`
- Test: `places.rs` `#[cfg(test)]`

**Interfaces:**

```rust
pub struct PlaceQuery {
    pub surface: String,
    pub search_keys: Vec<String>,
}

pub fn place_query(surface: &str) -> PlaceQuery
```

`surface` is **cloned unchanged** (trim only outer whitespace). `search_keys` includes: trimmed surface, and if English leading `the ` / `The ` then also the remainder — **both** keys, never replacing surface. `The Hague` → surface `"The Hague"`, keys `["The Hague", "Hague"]`. Resolution later tries keys in order and must prefer a hit on `"The Hague"` if the gazetteer has it.

- [ ] **Step 1: Tests**

```rust
#[test]
fn the_hague_keeps_surface() {
    let q = place_query("The Hague");
    assert_eq!(q.surface, "The Hague");
    assert!(q.search_keys.iter().any(|k| k == "The Hague"));
}

#[test]
fn the_united_states_adds_key_without_article() {
    let q = place_query("the United States");
    assert_eq!(q.surface, "the United States");
    assert!(q.search_keys.iter().any(|k| k == "United States"));
}
```

- [ ] **Step 2–4: Implement, test, commit** `feat(quality): preserve place surface when building search keys`

---

### Task 5: Migration 027 (structure only)

**Files:**
- Create: `migrations/027_unified_person_pipeline.sql`
- Modify: none of the live tables’ data

**Interfaces:** none (SQL). After this task, `cargo build -p talaria-api` so sqlx embeds the file.

SQL must:

1. Drop `canonical_events_pipeline_check`; add `CHECK (pipeline = 'person')`; `ALTER COLUMN pipeline SET DEFAULT 'person'`.
2. Drop indexes/constraints that mention `pipeline = 'quality'` or `pipeline = 'person'` uniqueness that would duplicate, then recreate the useful ones as `WHERE is_active AND pipeline = 'person'` (map eligible, occurrence stem, subject/type/time, timeline eligible, `uq_canonical_active_occurrence` on `(entity_id, occurrence_key)`, singleton birth/death, fingerprint unique). Drop `uq_canonical_events_active_person_occurrence`.
3. `CREATE UNIQUE INDEX uq_entities_qid ON entities (qid) WHERE qid IS NOT NULL;` — if duplicates exist this index **fails**. Do **not** merge rows here. Document in SQL comment: rebuild command merges first. For **dev** that currently has Q7186×3, the migrate step will fail until Task 11 rebuild merge — **order: ship merge function in Task 6/11 before applying unique index OR** make Task 5 create the index only if no dupes, else skip with a `DO` block that RAISES NOTICE. Preferred: Task 5 adds a **non-unique** index `idx_entities_qid` and Task 11’s rebuild creates the UNIQUE after merge. **Do the preferred path:** 027 = `CREATE INDEX IF NOT EXISTS idx_entities_qid ON entities (qid);` plus comment. Unique comes in rebuild after merge, as a second SQL file `028_entities_qid_unique.sql` applied **by the rebuild command** after merge, not on every environment blindly… Spec said unique in 027. Compromise that won’t break migrate: 027 unique index in a `DO` block: if `(SELECT count(*) FROM (SELECT qid FROM entities WHERE qid IS NOT NULL GROUP BY qid HAVING count(*)>1) s) = 0` then create unique; else NOTICE. Rebuild later creates it after merge.

4. `event_candidates`: `ALTER snapshot_id DROP NOT NULL`; `ALTER fragment_id DROP NOT NULL`; add `raw_document_id UUID REFERENCES raw_documents(id) ON DELETE SET NULL`; add CHECK `(snapshot_id IS NOT NULL OR raw_document_id IS NOT NULL)`.

5. `event_evidence`: add `evidence_hash TEXT`; add `source_locator TEXT`; backfill `evidence_hash = md5(canonical_event_id::text || coalesce(raw_document_id::text,'') || coalesce(quoted_text,''))` for existing rows; `CREATE UNIQUE INDEX uq_event_evidence_hash ON event_evidence (canonical_event_id, coalesce(raw_document_id, '00000000-0000-0000-0000-000000000000'::uuid), evidence_hash) WHERE evidence_hash IS NOT NULL;` — Postgres unique with coalesce is OK. Simpler: `UNIQUE (canonical_event_id, raw_document_id, evidence_hash)` allowing multiple NULLs in SQL unique — **bad for null raw_document_id**. Use the coalesced unique index.

6. **No TRUNCATE. No rejection_codes on canonical_events.**

- [ ] **Step 1: Write the SQL file** as specified. Header comment: `Structure only. Purge is talaria admin rebuild-person-pipeline.`

- [ ] **Step 2: `cargo build -p talaria-api`** so the migration is embedded.

- [ ] **Step 3: Apply on dev** `cargo run -p talaria-api -- migrate` (or `serve` once). If unique QID index is skipped due to dupes, that is expected.

- [ ] **Step 4: Commit** `feat(store): person pipeline schema without data wipe`

Include `migrations/027_unified_person_pipeline.sql` only.

---

### Task 6: Idempotent evidence (kill `source_count++`)

**Files:**
- Modify: `crates/talaria-store/src/person_events.rs`
- Modify: `crates/talaria-store/src/lib.rs` (exports)
- Test: `crates/talaria-store/tests/person_evidence.rs` — **skip if no DATABASE_URL**; prefer a **pure** hash unit test in `person_events.rs` plus SQL that can run under `#[ignore]` if CI has no Postgres.

Prefer unit-testable:

```rust
pub fn evidence_hash(raw_document_id: Option<Uuid>, locator: &str, quote_or_statement: &str) -> String
// sha256 hex of "v1|{raw}|{locator}|{quote}"
```

Replace `reinforce_person_event` body: **delete the UPDATE**. Callers must only insert evidence. Remove the function or make it a deprecated no-op that `compile_error` — **delete it** and fix compile errors in `person_ingest.rs` in this same task (minimal: stop calling it; insert evidence only).

Change `insert_person_quote_evidence` to:

```sql
INSERT INTO event_evidence (
  canonical_event_id, sentence_id, quoted_text, raw_document_id,
  confidence, evidence_type, source_locator, evidence_hash
) VALUES ($1, NULL, $2, $3, $4, $5, $6, $7)
ON CONFLICT DO NOTHING
RETURNING id
```

`ON CONFLICT` requires a named constraint. Use the unique index from 027 — in PostgreSQL unique indexes can be used if you add `CONSTRAINT uq_event_evidence_dedup UNIQUE NULLS NOT DISTINCT (...)` on PG15+. PostGIS 16 image is PG16 — `NULLS NOT DISTINCT` works:

```sql
ALTER TABLE event_evidence
  ADD CONSTRAINT uq_event_evidence_dedup
  UNIQUE NULLS NOT DISTINCT (canonical_event_id, raw_document_id, evidence_hash);
```

Put that in 027 if not already. Then `ON CONFLICT ON CONSTRAINT uq_event_evidence_dedup DO NOTHING`.

If `RETURNING id` is empty, fetch existing id or return `Option<Uuid>`.

Do **not** update `canonical_events.source_count`. Read APIs that display counts should `COUNT(event_evidence)` later (YAGNI if UI does not show it).

- [ ] **Step 1: Add `evidence_hash` helper tests** in `person_events.rs`:

```rust
#[test]
fn same_inputs_same_hash() {
    let id = Uuid::nil();
    assert_eq!(
        evidence_hash(Some(id), "span:0-10", "quote"),
        evidence_hash(Some(id), "span:0-10", "quote")
    );
}

#[test]
fn different_quote_different_hash() {
    let id = Uuid::nil();
    assert_ne!(
        evidence_hash(Some(id), "span:0-10", "a"),
        evidence_hash(Some(id), "span:0-10", "b")
    );
}
```

- [ ] **Step 2–4: Implement hash, unique insert, delete `reinforce_person_event`, fix callers, test, commit** `fix(store): idempotent event evidence without mutating source_count`

Grep: `reinforce_person_event` must be zero matches after this task.

---

### Task 7: QID-first entity upsert

**Files:**
- Modify: `crates/talaria-store/src/entities.rs`
- Modify: `crates/talaria-api/src/cli_helpers.rs` — **do not** create entity from surface before QID in person ingest (person ingest will call a new helper). Leave `open_db_for_subject` for dump/legacy CLI.
- Test: unit tests cannot hit Postgres easily; test **pure** merge key normalisation:

```rust
pub fn normalize_qid(qid: &str) -> Option<String>
// trim, uppercase Q, require Q[0-9]+
```

Add:

```rust
pub async fn upsert_person_by_qid(
    pool: &PgPool,
    qid: &str,
    wikidata_label: &str,
    wiki_lang: &str,
    wikipedia_title: &str,
    typed_surface: &str,
) -> anyhow::Result<Uuid>
```

Logic: `find_entity_by_qid` → if hit, insert alias `typed_surface` if distinct, return id. Else `upsert_entity_from_wikidata` then alias.

Person ingest Task 8 **must** resolve QID (HTTP Wikidata) **before** this upsert. If QID missing, ingest returns error `qid_unresolved` (no `upsert_entity_surface`).

- [ ] **Step 1: Tests for `normalize_qid`** (`Some("Q517")` from `"q517"`, `None` from `"LotD"` / empty).

- [ ] **Step 2–4: Implement, export from `lib.rs`, commit** `feat(store): upsert person by QID before surface create`

---

### Task 8: Split `person_ingest` and persist candidates-first

**Files:**
- Create: `crates/talaria-api/src/person_ingest/mod.rs` (move `run_person_ingest`)
- Create: `crates/talaria-api/src/person_ingest/resolve.rs`
- Create: `crates/talaria-api/src/person_ingest/collect.rs` (move wiki fetch / follow queue from current file)
- Create: `crates/talaria-api/src/person_ingest/extract.rs` (LLM vs structured split — call existing `llm::extract_chunk` and WDQS persist helpers)
- Create: `crates/talaria-api/src/person_ingest/grounding.rs` (thin wrap `accept_items`; structured items skip verbatim quote)
- Create: `crates/talaria-api/src/person_ingest/typing.rs` (`time_to_json`, `place_query`, geocode via existing `geocode_place`)
- Create: `crates/talaria-api/src/person_ingest/gating.rs` (build `EventCandidate`, `apply_gates`, `classify_attribution`)
- Create: `crates/talaria-api/src/person_ingest/persist.rs`
- Delete: `crates/talaria-api/src/person_ingest.rs` after move
- Modify: `crates/talaria-api/src/main.rs` `mod person_ingest;`
- Modify: `crates/talaria-store/src/quality.rs` insert candidate — or add `insert_person_candidate` in `person_events.rs` that writes `event_candidates` with `raw_document_id`, nullable snapshot/fragment, status, rejection_codes.

**Interfaces:**

```rust
// persist.rs
pub enum PersistOutcome {
    Canonical { event_id: Uuid, inserted: bool },
    CandidateOnly { candidate_id: Uuid, status: CandidateStatus },
}

pub async fn persist_gated_item(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
    subject: &str,
    item: &talaria_quality::GroundedItem,
    decision: &talaria_quality::GateDecision,
    attribution: talaria_quality::AttributionMatch,
    raw_document_id: Uuid,
    occurrence_key: &str,
    time: &talaria_quality::TypedTime,
    coords: Option<(f64, f64)>,
    source_locator: &str,
) -> anyhow::Result<PersistOutcome>
```

Rules:

1. Always `INSERT event_candidates`.
2. If `GateDecision::Accept` **and** `auto_accept_attribution`: lookup `find_active_person_event_by_occurrence`; if some, insert evidence only (`inserted: false`); if none, `insert_person_event` using `time_to_json` / `start_time_from_typed` (never `{kind: year}`).
3. If `NeedsReview` or `Reject`: **no** canonical insert.
4. Two-pass: pass 1 Wikidata birth/death into `GateContext` before pass 2.

`run_person_ingest` signature stays:

```rust
pub async fn run_person_ingest(
    config: &AppConfig,
    subject: &str,
    qid: Option<&str>,
    wiki_lang: &str,
    max_documents: u32,
    seed_list: Option<&Path>,
) -> anyhow::Result<Value>
```

Resolve QID first (`resolve.rs`); fail if none.

Pass 1: Wikidata statements → birth/death years only (reuse `fetch_wikidata_subject_meta`).

Stop writing `{ "kind": "year", "year": item.year }` in persist.

**Grounding two modes:** in `grounding.rs`, if extractor is Wikidata/WDQS, treat `quote_is_grounded` as true when `statement_id` is present; store locator JSON in `source_locator`.

Keep HTTP collect/LLM behaviour from the current file (do not rewrite fetch). This task is a move + persist/gate change.

- [ ] **Step 1: Add a unit test** in `gating.rs` or `crates/talaria-api/src/person_ingest/gating.rs`:

```rust
#[test]
fn louis_xvi_1981_anecdote_rejects() {
    // build EventCandidate year 1981 type anecdote, ctx death 1793
    // assert GateDecision::Reject contains EventAfterSubjectDeath
}
```

This can live in `talaria-quality` if already covered by Task 2 — then Task 8 test is compile: `cargo build -p talaria-api`.

- [ ] **Step 2: `cargo build -p talaria-api`** after the split. Fix imports. `run_explorer_lane` must still compile.

- [ ] **Step 3: Change `run_explorer_lane`** in `crates/talaria-api/src/routes/ingest.rs`: call **only** `person_ingest::run_person_ingest`. **Remove** `run_ingest_quality` sister-wiki call (that was the quality noise path on search-bar ingest).

- [ ] **Step 4: `cargo test -p talaria-quality --lib && cargo build -p talaria-api`**

- [ ] **Step 5: Commit** `feat(api): candidate-first person ingest modules`

---

### Task 9: HTTP timeline default `person` + retire old pipelines

**Files:**
- Modify: `crates/talaria-api/src/routes/events.rs` (`default_pipeline`, `resolve_pipeline`, `event_to_json`, `geojson_feature`, tests)
- Modify: `crates/talaria-store/src/canonical_events.rs` — extend `CanonicalEventRow` + SELECT to include `time_json` (jsonb). If adding a column to `query_as`, use `sqlx::types::Json<Value>` or `serde_json::Value` with `#[sqlx(json)]`.

**Interfaces:**

```rust
fn default_pipeline() -> Option<String> { Some("person".into()) }

fn resolve_pipeline(explicit: &Option<String>) -> Result<Option<String>, RetiredPipeline>
```

If explicit is `quality` or `legacy`, handlers return `Json` 400:

```json
{ "error": "pipeline_retired", "use": "person" }
```

Use `axum` `StatusCode::BAD_REQUEST`. Change handler signatures from `Json<Value>` to `impl IntoResponse` if needed.

`event_to_json`: drop `confidence`; add `"time": event.time_json`. Keep `start_time` for sort compatibility but frontend must not slice it (Task 10).

Update test `timeline_and_map_default_to_the_quality_pipeline` → `…_person_pipeline` asserting `"person"`. Add test that `resolve_pipeline(&Some("quality".into()))` is `Err`.

- [ ] **Step 1: Change the existing unit test** so it fails (expects `quality`).

- [ ] **Step 2: Run** `cargo test -p talaria-api timeline_and_map_default -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Implement default, 400, payload. SELECT `ce.time_json`.**

- [ ] **Step 4: `cargo test -p talaria-api --lib`**

- [ ] **Step 5: Commit** `feat(api): default timeline pipeline person and retire quality query`

---

### Task 10: Frontend pipeline + typed time display

**Files:**
- Modify: `web/src/lib/api.ts` (`TimelineEvent`, `timelineSearchParams`)
- Modify: `web/src/features/events/mappers/timeline.ts`
- Modify: `web/src/lib/geo.ts` if `formatDateLabel` / `filterTimelineUntilYear` only read `start_time` — keep using `start_time` for histogram **or** prefer `event.time.start` year. Histogram may keep `start_time` projection. Display label: if `time.surface` present use it; else `time.start`.

**Interfaces:**

```ts
export interface EventTime {
  kind: "exact" | "range" | "approx" | "unknown";
  start?: string | null;
  end?: string | null;
  precision?: "day" | "month" | "year";
  calendar?: string;
  surface?: string | null;
}

export interface TimelineEvent {
  // existing fields minus requiring confidence
  time?: EventTime;
  start_time?: string | null;
  // confidence optional, do not send from API
}
```

`timelineSearchParams`: `pipeline: query.pipeline ?? "person"`.

`mapTimelineEventToItem`: year from `event.time?.start` first 4 chars if numeric, else `start_time`; drop assigning `confidence` (leave optional undefined).

- [ ] **Step 1: Change default pipeline string; `npm run build` in `web/`** will fail typecheck if `confidence` still required — make it optional.

- [ ] **Step 2: `cd web && npm run build`**

Expected: PASS.

- [ ] **Step 3: Commit** `feat(web): request person pipeline and render time.surface`

No browser verification unless API is up; typecheck is the gate.

---

### Task 11: `talaria admin rebuild-person-pipeline`

**Files:**
- Create: `crates/talaria-api/src/rebuild.rs`
- Modify: `crates/talaria-api/src/main.rs` clap: nested `admin rebuild-person-pipeline --confirm-destruction --backup-manifest PATH`

**Interfaces:**

```rust
pub async fn rebuild_person_pipeline(
    config: &AppConfig,
    confirm: bool,
    manifest_path: &Path,
) -> anyhow::Result<()>
```

Without `--confirm-destruction`, print counts and exit 2.

With confirm:

1. SQL counts: `canonical_events`, `event_evidence`, `event_candidates`, entities `canonical_name ILIKE '%LotD%'`, duplicate qids.
2. Write JSON manifest `{ counts, sampled_ids: [...] }`.
3. Merge duplicate QIDs: keep the row whose `canonical_name` equals Wikidata-ish longest / most events; `UPDATE` child FKs (`canonical_events.entity_id`, `event_candidates.subject_entity_id`, `entity_aliases`, `soft_claims` if entity_id exists) then `DELETE` losers. Then `DELETE FROM entities WHERE canonical_name ILIKE '%LotD%'`.
4. `TRUNCATE canonical_events CASCADE` (evidence goes); `TRUNCATE event_candidates`; do **not** truncate `soft_claims` (Agora).
5. `CREATE UNIQUE INDEX IF NOT EXISTS uq_entities_qid ON entities (qid) WHERE qid IS NOT NULL`.
6. Verify `canonical_events` count = 0; verify no duplicate qids; fail if unique index still cannot be created.

Do **not** auto reingest. Print next step: search-bar ingest or `talaria` person ingest.

- [ ] **Step 1: Wire clap; `cargo run -p talaria-api -- admin rebuild-person-pipeline` without flag**

Expected: exit 2, prints counts (needs DATABASE_URL).

- [ ] **Step 2: Implement merge + truncate behind flag.**

- [ ] **Step 3: Commit** `feat(api): explicit rebuild-person-pipeline wipe`

---

### Task 12: AGENTS.md + explorer wiring check

**Files:**
- Modify: `AGENTS.md` — replace “Quality pipeline coexistence”, “Lot E coexistence is strict”, and `source_count++` with: single `pipeline='person'`; candidates vs canonical; evidence idempotent; typed time kind/precision; rebuild CLI; HTTP default person; `pipeline=quality` 400.
- Keep dump/legacy CLI as **offline dump tools**, not as explorer API.
- Grep `source_count++` and `pipeline='quality'` in `AGENTS.md` / `README.md` if README claims explorer shows quality — fix README one paragraph only if it would mislead (user said no extra md; README is existing product doc — update the explorer pipeline sentence only).

- [ ] **Step 1: Edit AGENTS.md** so a future agent cannot restore three lanes.

- [ ] **Step 2: Grep** `run_ingest_quality` in `routes/ingest.rs` — must be absent from explorer lane (Task 8).

- [ ] **Step 3: `cargo test -p talaria-quality --lib && cargo test -p talaria-api --lib && cargo clippy -p talaria-quality -p talaria-api --all-targets --all-features`**

- [ ] **Step 4: Commit** `docs: single person pipeline is the explorer contract`

---

### Task 13: Fixture regression tests (no network)

**Files:**
- Create: `crates/talaria-quality/tests/person_pipeline_regressions.rs` for pure gate/attribution/time cases already split — **fold remaining into this file** only if not duplicated:

| Case | Assert |
|---|---|
| Louis XVI anecdote 1981 | `event_after_subject_death` |
| Victor Hugo battle followed unattributed | `Unattributed` |
| Columbus arrival 1453 | `event_before_subject_birth` |
| The Hague surface | unchanged |
| time_json kind never year | Task 1 |

Add `crates/talaria-store/src/person_events.rs` tests already in Task 6.

If a Postgres test env exists, optional ignored test: insert same evidence twice, `COUNT(*)` = 1.

- [ ] **Step 1: One integration file** that imports public `talaria_quality` APIs and restates the measured DB cases.

- [ ] **Step 2: `cargo test -p talaria-quality`**

- [ ] **Step 3: Commit** `test(quality): lock person-pipeline regression cases`

---

## Self-review (spec coverage)

| Spec section | Task |
|---|---|
| Seven-step pipeline / modules | 8 |
| Two passes birth/death | 8 |
| Candidates vs canonical | 8 (+ 5 schema) |
| Role-aware lifespan | 2 |
| SubjectAttribution | 3 |
| Append-only evidence | 6 |
| time_json kind/precision | 1, 9, 10 |
| Provenance locators | 5 (`source_locator`), 8 grounding |
| Place surface | 4 |
| QID alignment | 7, 11 |
| Migration no TRUNCATE | 5 |
| Rebuild CLI | 11 |
| API default + 400 | 9 |
| Frontend | 10 |
| AGENTS.md | 12 |
| Tests list | 1–4, 6, 13 |
| Confidence hidden | 9, 10 |
| Do not delete lot_e.rs | 8, 12 (HTTP only) |
| Unique QID after merge | 5 skip if dupes, 11 creates unique |
| BFS budget / concurrent ingest | existing collect code + unique occurrence index; no new BFS rewrite |
| LLM version on run | record `openai_model` env in ingest JSON report in Task 8 `run_person_ingest` output (`"llm_model": …`) — add one field in the existing `json!({...})` report |

**Placeholders:** none intended. `persist_gated_item(...)` ellipsis in Task 8 is filled by using `GroundedItem` + `GateDecision` + pool + `entity_id` + `occurrence_key` already in current `persist_fact`.

**Type names:** `AttributionMatch`, `place_query`, `upsert_person_by_qid`, `evidence_hash`, `pipeline_retired` used consistently.

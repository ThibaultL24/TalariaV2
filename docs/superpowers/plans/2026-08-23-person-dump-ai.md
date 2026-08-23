# Person dump → AI → Explorer / Agora Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One ingest per person that dumps raw Wikipedia/Wikidata (then academic notices), lets a grounded LLM emit map facts and Agora debates, and shows only `pipeline='person'` in the Explorer.

**Architecture:** Pure grounding + parse in `talaria-quality`. Persistence in `talaria-store` (`pipeline='person'`). Orchestration in `talaria-api` (`person_ingest.rs`) replacing regex density on the Explorer lane. OpenAI is a swappable `LlmExtract` trait; tests use a mock. Gazetteer stays the only source of coordinates.

**Tech Stack:** Rust (sqlx, reqwest, serde), OpenAI Responses API (`OPENAI_API_KEY`), Postgres/PostGIS, Vite Explorer (`web/src/lib/api.ts`).

## Global Constraints

- Quote must be a substring of stored document text or the item is dropped (code-checked).
- LLM never emits lat/lon; gazetteer / Wikidata P625 only.
- `lane=debate` never creates `map_eligible` events.
- Do not requalify `pipeline='legacy'` or `'quality'` into `'person'`.
- No density floor. Agent of a fact must be the ingest subject.
- Explorer API default `pipeline=person`.
- Reuse Talaria v1 `OPENAI_API_KEY` / `OPENAI_MODEL` (default `gpt-5.4`).

**Spec:** `docs/superpowers/specs/2026-08-23-person-dump-ai-design.md`

## File map

| File | Responsibility |
|------|----------------|
| `crates/talaria-quality/src/grounding.rs` | Quote substring + agent heuristic + item validation |
| `migrations/022_person_pipeline.sql` | `pipeline='person'`, nullable `event_evidence.sentence_id`, `raw_document_id` |
| `crates/talaria-store/src/person_events.rs` | Insert person events + quote evidence |
| `crates/talaria-api/src/llm.rs` | `LlmExtract` trait + OpenAI JSON extract |
| `crates/talaria-api/src/person_ingest.rs` | Wave 1 dump + extract + persist |
| `crates/talaria-api/src/routes/ingest.rs` | Explorer job calls `run_person_ingest` |
| `crates/talaria-api/src/routes/events.rs` + `web/src/lib/api.ts` | Default pipeline `person` |

---

### Task 1: Grounding (quote + subject)

**Files:**
- Create: `crates/talaria-quality/src/grounding.rs`
- Modify: `crates/talaria-quality/src/lib.rs` (mod + re-exports)

**Produces:**
- `quote_is_grounded(document: &str, quote: &str) -> bool`
- `agent_is_other_person(quote: &str, subject: &str) -> bool`
- `GroundedItem { lane, event_type, role, year, place_surface, summary, quoted_text, confidence }`
- `validate_item(item, document, subject) -> Result<GroundedItem, RejectReason>`

- [ ] **Step 1: Write failing tests** in `grounding.rs` under `#[cfg(test)]`:

```rust
#[test]
fn quote_must_be_substring() {
    assert!(quote_is_grounded("Born in Warsaw in 1867.", "Born in Warsaw in 1867."));
    assert!(!quote_is_grounded("Born in Warsaw.", "Born in Paris."));
}

#[test]
fn drops_other_person_as_agent() {
    let q = "On 6 April 1920, Schrödinger married Annemarie Bertel.";
    assert!(agent_is_other_person(q, "Marie Curie"));
    assert!(!agent_is_other_person(
        "In 1891 Curie moved to Paris.",
        "Marie Curie"
    ));
}

#[test]
fn debate_lane_not_a_map_fact() {
    let item = raw_item("debate", "Born in Warsaw.", "Marie Curie was born in Warsaw.");
    let v = validate_item(&item, "Marie Curie was born in Warsaw.", "Marie Curie").unwrap();
    assert_eq!(v.lane, Lane::Debate);
}
```

- [ ] **Step 2: Run** `cargo test -p talaria-quality grounding -- --nocapture`  
  Expected: FAIL (module missing)

- [ ] **Step 3: Implement** `quote_is_grounded` via case-insensitive whitespace-normalized contains. `agent_is_other_person`: if quote has a capitalized two-word name that is not the subject / surname, treat as other agent. `validate_item` requires grounded quote; rejects `lane=fact` when `agent_is_other_person`.

- [ ] **Step 4: Re-run tests** — PASS

- [ ] **Step 5: Commit** `feat: ground LLM extracts in document quotes`

---

### Task 2: Schema `pipeline='person'`

**Files:**
- Create: `migrations/022_person_pipeline.sql`

```sql
ALTER TABLE canonical_events DROP CONSTRAINT IF EXISTS canonical_events_pipeline_check;
ALTER TABLE canonical_events
    ADD CONSTRAINT canonical_events_pipeline_check
    CHECK (pipeline IN ('legacy', 'quality', 'person'));

CREATE UNIQUE INDEX IF NOT EXISTS uq_canonical_events_active_person_occurrence
    ON canonical_events (occurrence_key)
    WHERE is_active AND occurrence_key IS NOT NULL AND pipeline = 'person';

ALTER TABLE event_evidence ALTER COLUMN sentence_id DROP NOT NULL;
ALTER TABLE event_evidence
    ADD COLUMN IF NOT EXISTS raw_document_id UUID REFERENCES raw_documents(id) ON DELETE SET NULL;
```

- Modify: `crates/talaria-store/src/quality.rs` insert may keep `pipeline='quality'` hardcoded; do not change quality inserts.

- [ ] **Step 1: Add migration file**

- [ ] **Step 2: `cargo build -p talaria-store`** — PASS (sqlx embed)

- [ ] **Step 3: Commit** `feat: allow pipeline=person and quote-only event evidence`

---

### Task 3: Persist person facts + debates

**Files:**
- Create: `crates/talaria-store/src/person_events.rs`
- Modify: `crates/talaria-store/src/lib.rs` (mod + pub use)
- Modify: `crates/talaria-store/src/canonical_events.rs` — `list_timeline_events` / `list_geojson_events` already filter `is_active` + `pipeline`; no change except default callers.

**Produces:**
```rust
pub struct PersonEventInsert { /* same fields as QualityEventInsert but pipeline person */ }
pub async fn insert_person_event(pool: &PgPool, e: &PersonEventInsert) -> anyhow::Result<Uuid>;
pub async fn insert_person_quote_evidence(
    pool: &PgPool,
    event_id: Uuid,
    quoted_text: &str,
    raw_document_id: Option<Uuid>,
    confidence: f64,
) -> anyhow::Result<Uuid>;
pub async fn find_active_person_event_by_occurrence(
    pool: &PgPool,
    occurrence_key: &str,
) -> anyhow::Result<Option<Uuid>>;
```

Evidence SQL: `INSERT INTO event_evidence (canonical_event_id, sentence_id, quoted_text, raw_document_id, confidence, evidence_type) VALUES ($1, NULL, $2, $3, $4, 'llm_quote')`.

Reinforce: if occurrence exists, `UPDATE canonical_events SET source_count = source_count + 1` (do not insert duplicate).

Debates: `insert_claim` with `claim_kind: "controversy"` or `"theory"` so `list_exportable_soft_claims` picks them up.

- [ ] **Step 1:** Unit-test occurrence reinforce vs insert with sqlx skipped if no DB; at minimum compile + `cargo test -p talaria-store --lib`

- [ ] **Step 2: Implement inserts** (copy `insert_quality_canonical_event` SQL, bind `'person'` instead of `'quality'`). `event_candidate_id` may be `Uuid::nil()` — if FK requires a real candidate, insert a stub candidate or make column nullable for person. **If FK fails:** insert a dummy `event_candidates` row with `extractor_version='llm_grounded'`.

- [ ] **Step 3: Commit** `feat: persist pipeline=person events with quote evidence`

---

### Task 4: LLM extract trait

**Files:**
- Modify: `crates/talaria-api/src/llm.rs`

**Produces:**
```rust
#[async_trait]
pub trait LlmExtract: Send + Sync {
    async fn extract(&self, subject: &str, document_title: &str, chunk: &str) -> anyhow::Result<Vec<RawLlmItem>>;
}

pub struct OpenAiExtract { /* key, model, client */ }
pub struct MockExtract { pub items: Vec<RawLlmItem> }

pub struct RawLlmItem {
    pub lane: String, // "fact" | "debate"
    pub event_type: String,
    pub role: String, // "direct" | "indirect"
    pub year: Option<i32>,
    pub place_surface: Option<String>,
    pub summary: String,
    pub quoted_text: String,
    pub confidence: f64,
}
```

OpenAI call: Responses API, `input` = system+user, ask for JSON array only. Parse with `serde_json`. On HTTP error, return `Err` (caller continues other chunks).

Prompt (user message): subject, title, chunk, instructions: only quotes from chunk; facts about subject; debates for controversies; commemorations allowed.

- [ ] **Step 1:** Test `MockExtract` returns seeded items; test JSON parse helper on a fixture string.

- [ ] **Step 2: Implement** `parse_llm_items(json: &str) -> Vec<RawLlmItem>`

- [ ] **Step 3: Commit** `feat: grounded OpenAI extract trait for person ingest`

---

### Task 5: Person ingest orchestrator (wave 1)

**Files:**
- Create: `crates/talaria-api/src/person_ingest.rs`
- Modify: `crates/talaria-api/src/main.rs` (`mod person_ingest;`)
- Modify: `crates/talaria-api/src/routes/ingest.rs` — `LANE_EXPLORER` calls `run_person_ingest` instead of `run_lot_e_density_ingest`

**Wave 1 only in this task:**
1. Upsert entity (already done by ingest route).
2. Fetch Wikipedia extract for subject title (reuse existing Wikipedia fetch in `lot_e.rs` / sources — extract a function if needed, do not copy 2000 lines).
3. Store as `raw_documents` (`source_kind='wikipedia'`, uri = wiki URL, body in `payload`).
4. Chunk text ~3000 chars on paragraph boundaries.
5. `extractor.extract(...)` then `validate_item`.
6. Facts: gazetteer `resolve_place_offline(place_surface)`; `map_eligible` iff coords; `occurrence_key` via `talaria_quality::occurrence_key`.
7. Debates: `insert_claim`.
8. Return JSON report `{ facts_inserted, facts_reinforced, debates_inserted, chunks, dropped }`.

Wikidata P-statements: if a small helper already exists in `wikidata_ingest.rs`, call it for birth/death/residence as extra facts **with quote = statement label**; still no regex on foreign bios.

Do **not** follow other person bios. Follow only if `is_followable_map_title` (places/battles) **and** the clause names the subject — skip in v1 if that helper is entangled; wave 1 = subject page only.

- [ ] **Step 1:** Integration-style unit test with `MockExtract` + fixture `fixtures/dumps/Marie Curie.txt`: Schrödinger marriage in fixture must yield 0 facts; a Curie+Warsaw sentence yields ≥1 fact. Use `sqlx::test` only if test DB available; otherwise test the pure loop `items_from_chunk` without DB.

Extract loop into:

```rust
pub fn accept_items(
    subject: &str,
    document: &str,
    raw: Vec<RawLlmItem>,
) -> Vec<GroundedItem>
```

Test that without Postgres.

- [ ] **Step 2: Wire** `run_explorer_lane` → `person_ingest::run_person_ingest`. Keep `run_lot_e_density_ingest` callable from CLI for now; Explorer HTTP must not call it.

- [ ] **Step 3:** `cargo test -p talaria-quality grounding` and `cargo test -p talaria-api accept_items`

- [ ] **Step 4: Commit** `feat: explorer ingest dumps wiki and lets LLM emit person facts`

---

### Task 6: API + UI default `person`

**Files:**
- Modify: `crates/talaria-api/src/routes/events.rs` — `default_pipeline() -> Some("person")`
- Modify: `web/src/lib/api.ts` — `pipeline: query.pipeline ?? "person"`
- Tests: extend `search_collapse` or add `#[cfg(test)]` on `default_pipeline` if extracted.

- [ ] **Step 1:** Change defaults.

- [ ] **Step 2:** `cargo test -p talaria-api` (existing search tests still pass)

- [ ] **Step 3:** `cd web && npm run build`

- [ ] **Step 4: Commit** `fix: Explorer reads pipeline=person by default`

---

### Task 7: Wave 2 academic dumps (same job)

**Files:**
- Modify: `crates/talaria-api/src/person_ingest.rs`
- Reuse: `run_agora_lane` / `corpus_ingest` for OpenAlex/theses.fr **after** wave 1.

After wave 1 report is ready, call existing corpus ingest with `corpus_limit` (already on the job). For each new `corpus_documents` abstract, run `accept_items` + persist debates (and facts only if quote names subject + place/year).

If live corpus fails, log and leave wave 1 facts.

- [ ] **Step 1:** Append wave 2 to `run_person_ingest`; report includes `corpus_documents`.

- [ ] **Step 2: Commit** `feat: person ingest wave 2 academic notices into Agora`

---

### Task 8: Manual check

- [ ] Restart API + Vite.
- [ ] `POST /api/v1/ingest/explorer` `{ "subject": "Marie Curie", "live": true }`.
- [ ] GeoJSON `pipeline=person`: no Verdun/Schrödinger; Warsaw/Paris present if wiki+LLM agree.
- [ ] Event detail shows `quoted_text`.

---

## Spec coverage

| Spec | Task |
|------|------|
| Quote substring | 1 |
| Other-person agent drop | 1, 5 |
| `pipeline=person` | 2, 3, 6 |
| Gazetteer-only coords | 5 |
| Debates → Agora/soft_claims | 3, 5, 7 |
| Explorer ingest = this job | 5 |
| Wave 1 then 2 | 5, 7 |
| Do not migrate legacy/quality | 3 (new writes only) |
| Commemoration type | 1 (`event_type` passthrough) |
| OPENAI_API_KEY | 4 |

## Out of this plan

- PDF full text
- UI toggle life vs commemorations
- On-chain Intuition publish
- Deleting `lot_e.rs`

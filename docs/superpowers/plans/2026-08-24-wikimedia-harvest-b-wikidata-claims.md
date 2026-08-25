# Wikimedia harvest B — Wikidata full claims Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist full Wikidata claims (qualifiers, ranks, references, typed time including BCE) and promote Events only from a closed PID list.

**Architecture:** Snapshot stores entity JSON. New table `wikibase_statements`. `talaria-wikidata` parses time + promotion. Connector stops treating TSV `STATEMENT` as canonical. `structured_statement` extractor runs only on promoted lines. Quality gates unchanged.

**Tech Stack:** Rust, sqlx, `talaria-wikidata`, `talaria-sources`, `talaria-store`, `talaria-api`.

## Global Constraints

- Person-first: `wbgetentities` for subject QID; dump filtered to neighborhood, never `latest-all` by default.
- STATEMENT TSV may remain as a **derived** fixture format.
- Deprecated ranks stored, excluded from active projection.
- Preferred beats normal for P569/P570/P19/P20.
- No live WDQS in tests.
- Do not change quality gates / `occurrence_key`.

---

### Task 1: Typed Wikibase time + promotion

**Files:**
- Create: `crates/talaria-wikidata/src/time.rs`
- Create: `crates/talaria-wikidata/src/promote.rs`
- Modify: `crates/talaria-wikidata/src/lib.rs`

**Interfaces:**
```rust
pub struct WikibaseTime { pub year: i32, pub precision: i32, pub calendar: String }
pub fn parse_wikibase_time(time: &str, precision: Option<i32>, calendar: Option<&str>) -> Option<WikibaseTime>;
/// Closed list. `pid` is "P551". `has_date` true if P580/P582/P585/P569/P570 present on statement or mainsnak time.
pub fn promote_event(pid: &str, has_date: bool, subject_is_participant: bool) -> Option<(&'static str, &'static str)>;
// returns (event_type, predicate) or None → Claim only
```

Promotion table (copy from spec):

| Promote | Stay Claim |
|---|---|
| P569 / P570 (P570 only if present — caller passes has_date) | P551 / P937 without date |
| P793 + date | P106, P27, P18, ids |
| P39 / P26 / P69 + date | P19/P20 alone |
| P607 / P1344 + date + participant | any other dated property |

- [ ] **Step 1: Failing tests** in `time.rs` and `promote.rs`

```rust
#[test]
fn bce_year_negative() {
    let t = parse_wikibase_time("-0044-03-15T00:00:00Z", Some(11), None).unwrap();
    assert_eq!(t.year, -44);
}

#[test]
fn ce_year() {
    let t = parse_wikibase_time("+1769-08-15T00:00:00Z", Some(11), None).unwrap();
    assert_eq!(t.year, 1769);
}

#[test]
fn p551_without_date_is_claim() {
    assert!(promote_event("P551", false, true).is_none());
}

#[test]
fn p551_with_date_is_residence() {
    assert_eq!(promote_event("P551", true, true), Some(("residence", "resided_in")));
}

#[test]
fn p106_never_event() {
    assert!(promote_event("P106", true, true).is_none());
}
```

For P551+date: spec says P551 without P580/P582/P585 stays Claim. **With** those qualifiers it becomes Event. Map P551+date → `residence`/`resided_in`. P569 → birth/born_in, P570 → death/died_in, P793 → notable_event/occurred, P39 → office/held_office, P26 → marriage/married, P69 → education/studied_at, P607/P1344 → battle/fought_at (or conflict/participated_in).

- [ ] **Step 2: `cargo test -p talaria-wikidata bce_year_negative` — FAIL**
- [ ] **Step 3: Implement `parse_wikibase_time`**

Do **not** take `time[0..4]`. Strip `+`; if the first char is `-`, parse the year as negative. Year field is everything before the first `-` after the optional sign (ISO: `+/-YYYY-MM-DD`). Handle years with more than 4 digits.

- [ ] **Step 4: Implement `promote_event` as a match on pid**
- [ ] **Step 5: `cargo test -p talaria-wikidata` PASS**
- [ ] **Step 6: Commit** `feat(wikidata): typed time including BCE and closed event promotion`

---

### Task 2: `wikibase_statements` table + persist

**Files:**
- Create: `migrations/024_wikibase_statements.sql`
- Create: `crates/talaria-store/src/wikibase.rs`
- Modify: `crates/talaria-store/src/lib.rs` (`mod wikibase; pub use`)

**SQL:**

```sql
CREATE TABLE IF NOT EXISTS wikibase_statements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    qid TEXT NOT NULL,
    guid TEXT NOT NULL,
    property TEXT NOT NULL,
    rank TEXT NOT NULL,
    snaktype TEXT NOT NULL,
    value_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    qualifiers_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    references_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    revision_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (guid, revision_id)
);
CREATE INDEX IF NOT EXISTS idx_wikibase_statements_qid ON wikibase_statements (qid);
CREATE INDEX IF NOT EXISTS idx_wikibase_statements_pid ON wikibase_statements (qid, property);
```

```rust
pub struct WikibaseStatementInsert {
    pub qid: String,
    pub guid: String,
    pub property: String,
    pub rank: String,
    pub snaktype: String,
    pub value_json: serde_json::Value,
    pub qualifiers_json: serde_json::Value,
    pub references_json: serde_json::Value,
    pub revision_id: Option<String>,
}
pub async fn upsert_wikibase_statement(pool: &PgPool, row: &WikibaseStatementInsert) -> anyhow::Result<Uuid>;
```

- [ ] **Step 1: Migration + store functions**
- [ ] **Step 2: `cargo check -p talaria-store`**
- [ ] **Step 3: Commit** `feat(store): wikibase_statements table`

---

### Task 3: Parse entity JSON → statements + derived STATEMENT lines

**Files:**
- Create: `crates/talaria-wikidata/src/claims.rs`
- Modify: `crates/talaria-sources/src/connectors/wikidata.rs` (`claims_to_text` → use parser)
- Modify: `crates/talaria-api/src/lot_e.rs` (same flattening; replace with parser)
- Test: `crates/talaria-wikidata/src/claims.rs`

**Interfaces:**
```rust
pub struct ParsedStatement {
    pub insert: /* fields matching WikibaseStatementInsert without pool */,
    pub event: Option<(String, String, Option<i32>, Option<String>)>, // type, pred, year, place_qid
}
pub fn parse_entity_claims(entity: &serde_json::Value) -> Vec<ParsedStatement>;
pub fn promoted_statement_lines(parsed: &[ParsedStatement]) -> String; // STATEMENT\t...
```

Skip `rank == "deprecated"` in `promoted_statement_lines`. For identity P569/P570/P19/P20, if any `preferred` exists, ignore `normal`.

`somevalue`/`novalue`: store statement (`snaktype`), `event: None`.

- [ ] **Step 1: Fixture test with mini Q517 JSON** (inline): P551 with P580/P582, P106 occupation. Assert two statements stored-shape; only P551 yields `event == Some(("residence", ...))`; P106 `event is None`. Year 1804 from qualifier.

- [ ] **Step 2: FAIL then implement `parse_entity_claims`**

Walk `entity["claims"]` objects. GUID from `id`. Mainsnak `snaktype`, `property`, `datavalue`. Qualifiers/references as raw JSON.

Date detection: mainsnak time **or** qualifier P580/P582/P585.

Place: qualifier P276 or mainsnak item id.

- [ ] **Step 3: Change Wikidata connector `fetch`**: `text` = `promoted_statement_lines(...)`; `raw_metadata` = full entity (already). `content_type` = `application/vnd.wikibase.entity+json` internally; keep text for extractors.

- [ ] **Step 4: Replace `lot_e` statement flattening** with `parse_entity_claims` + `promoted_statement_lines`. Persist statements via `upsert_wikibase_statement` when pool is available in that path.

- [ ] **Step 5: `cargo test -p talaria-wikidata && cargo test -p talaria-sources && cargo test -p talaria-api`**

Existing fixture tests that feed `STATEMENT\tmarriage...` must still pass (`structured_statement` still parses TSV).

- [ ] **Step 6: Commit** `feat(wikidata): persist full claims; STATEMENT lines only for promoted events`

---

### Task 4: Dump neighborhood, not occupation-only humans

**Files:**
- Modify: `crates/talaria-wikidata/src/dump.rs`
- Test: existing dump test + new test with a tiny JSON array file

**Interfaces:**
```rust
pub fn stream_entities_for_qids(path: &Path, keep: &HashSet<String>, mut on_entity: impl FnMut(Value) -> Result<()>) -> Result<DumpIngestStats>;
```

Keep `stream_humans` for current callers. Add neighborhood stream: if `keep` contains entity id, emit **full entity JSON** (not occupation-only struct).

Test: tempfile with 2 items (Q5 human with P551 + a Qid place). `keep = {human}`; callback receives full claims.

- [ ] Implement + `cargo test -p talaria-wikidata`
- [ ] Commit `feat(wikidata): dump filter by QID neighborhood with full claims`

---

## Spec coverage (B)

| Spec | Task |
|---|---|
| full claims + qualifiers/refs/rank/snaktype | 2–3 |
| typed time / BCE | 1 |
| closed promotion list | 1, 3 |
| dump neighborhood | 4 |
| STATEMENT derived | 3 |
| somevalue/novalue no Event | 3 |
| deprecated excluded from projection | 3 |

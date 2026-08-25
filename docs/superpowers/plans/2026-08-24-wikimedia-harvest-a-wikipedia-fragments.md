# Wikimedia harvest A — Wikipedia wikitext fragments Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Store Wikipedia snapshots as wikitext and persist section/infobox/sentence fragments with links and refs, instead of one plaintext blob.

**Architecture:** `talaria-text` builds a fragment list from wikitext. Live fetch (`lot_e` + Wikipedia connector) keeps wikitext as `document_snapshots.text`. `talaria-store` stores fragment `metadata` JSON. Extractors still run on fragment/plain text; gates unchanged.

**Tech Stack:** Rust, sqlx migrations, `talaria-text`, `talaria-sources`, `talaria-store`, `talaria-api`.

## Global Constraints

- Person-first harvest; languages `fr` and `en` only.
- Snapshot authority is wikitext + `revision_id`.
- No live network in tests (fixtures only).
- Do not change quality gates, `occurrence_key`, or append-only events.
- Do not assemble Events in this phase.
- User-Agent already identifies Talaria; keep polite delays.

---

### Task 1: Wikitext fragment parser

**Files:**
- Create: `crates/talaria-text/src/fragments.rs`
- Modify: `crates/talaria-text/src/lib.rs`
- Modify: `crates/talaria-text/src/sections.rs` (add `start_offset` / `end_offset` on `WikiSectionSpan`)
- Test: `crates/talaria-text/src/fragments.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Consumes: `split_wiki_sections`, `split_sentences`, `wikitext_to_plain`, `extract_wikilinks`, `parse_infobox_fields`
- Produces:
  ```rust
  pub struct FragmentLink { pub surface: String, pub target_title: String, pub qid: Option<String> }
  pub struct FragmentCitation { pub ref_name: Option<String>, pub text: String }
  pub struct WikiContentFragment {
      pub kind: &'static str, // "section" | "infobox" | "sentence"
      pub parent_section_ordinal: Option<i32>,
      pub ordinal: i32,
      pub text: String,
      pub start_offset: i32,
      pub end_offset: i32,
      pub section_path: Vec<String>,
      pub internal_links: Vec<FragmentLink>,
      pub citations: Vec<FragmentCitation>,
  }
  pub fn extract_refs(wikitext: &str) -> Vec<FragmentCitation>;
  pub fn fragment_wikitext(wikitext: &str) -> Vec<WikiContentFragment>;
  ```
- Sentence `start_offset`/`end_offset` are character offsets into the **parent section’s wikitext slice**, plus that section’s `start_offset` in the full snapshot (so they land in `snapshot.text`). If a plain sentence cannot be located in wikitext (templates), use the parent section’s offsets.

- [ ] **Step 1: Write the failing test**

Add at the bottom of `fragments.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const NAP: &str = r#"{{Infobox Biographie2
|nom=Napoléon
|lieu naissance=[[Ajaccio]]
}}
Napoléon fut couronné à [[Paris]] le 2 décembre 1804.<ref>Tulard</ref>

== Consulat et Empire ==
Il régna jusqu'en 1814.
"#;

    #[test]
    fn fragments_napoleon_fr_fixture() {
        let frags = fragment_wikitext(NAP);
        assert!(frags.iter().any(|f| f.kind == "infobox"));
        assert!(frags.iter().any(|f| f.kind == "section" && f.section_path == ["Lead"]));
        assert!(frags.iter().any(|f| f.kind == "section" && f.section_path.iter().any(|s| s.contains("Consulat"))));
        let crown = frags.iter().find(|f| f.kind == "sentence" && f.text.contains("1804")).expect("crowning sentence");
        assert!(crown.internal_links.iter().any(|l| l.target_title == "Paris"));
        assert!(crown.citations.iter().any(|c| c.text.contains("Tulard")));
        assert!(frags.iter().filter(|f| f.kind == "sentence").count() >= 2);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p talaria-text fragments_napoleon_fr_fixture -- --nocapture`

Expected: FAIL (module `fragments` not declared / `fragment_wikitext` missing).

- [ ] **Step 3: Write minimal implementation**

1. Extend `WikiSectionSpan` with `start_offset: i32` and `end_offset: i32` computed while scanning the original string (use `wikitext.find(line)` / running byte cursor on the full source, convert to char offsets via `s[..byte].chars().count()`).
2. Implement `extract_refs`: scan `<ref>...</ref>` and `<ref name="x">...</ref>`; skip self-closing `<ref name="x" />`.
3. Implement `fragment_wikitext`:
   - If `{{Infobox` / `{{infobox` present, one `infobox` fragment with `parse_infobox_fields` text join and infobox wikilinks.
   - For each section: push `section` fragment (`text` = section wikitext, offsets into full source, `section_path` = `[title]`).
   - `split_sentences(&wikitext_to_plain(&section.wikitext))`; for each sentence push `sentence` with links from `extract_wikilinks` on the **section wikitext** filtered to those whose `display` or `target` appears in the sentence; citations whose text or preceding sentence contains the ref (attach refs whose source index falls inside the section).
4. `pub use fragments::{fragment_wikitext, extract_refs, WikiContentFragment, FragmentLink, FragmentCitation};` from `lib.rs`.
5. Fix any existing `split_wiki_sections` tests that construct `WikiSectionSpan` without the new fields.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p talaria-text`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/talaria-text
git commit -m "feat(text): fragment Wikipedia wikitext into sections, infobox, sentences"
```

---

### Task 2: Fragment schema (kinds + metadata)

**Files:**
- Create: `migrations/023_wiki_fragment_metadata.sql`
- Modify: `crates/talaria-store/src/quality.rs` (`DocumentFragmentInsert`, `insert_document_fragment`)
- Modify: `crates/talaria-store/src/lib.rs` if the insert signature is re-exported (already is)

**Interfaces:**
- Consumes: existing `document_fragments` table
- Produces: `DocumentFragmentInsert.metadata: serde_json::Value` (default `{}`)

- [ ] **Step 1: Write the migration**

```sql
-- migrations/023_wiki_fragment_metadata.sql
ALTER TABLE document_fragments DROP CONSTRAINT IF EXISTS document_fragments_fragment_kind_check;
ALTER TABLE document_fragments ADD CONSTRAINT document_fragments_fragment_kind_check
    CHECK (fragment_kind IN ('sentence', 'clause', 'section', 'infobox'));

ALTER TABLE document_fragments DROP CONSTRAINT IF EXISTS document_fragments_clause_check;
ALTER TABLE document_fragments ADD CONSTRAINT document_fragments_clause_check
    CHECK (
        (fragment_kind = 'clause' AND clause_index IS NOT NULL)
        OR (fragment_kind IN ('sentence', 'section', 'infobox') AND clause_index IS NULL)
    );

ALTER TABLE document_fragments
    ADD COLUMN IF NOT EXISTS metadata JSONB NOT NULL DEFAULT '{}'::jsonb;
```

If the first CHECK was inline unnamed, drop by querying `pg_constraint` in a comment and using the actual name from `migrations/006_document_snapshots.sql` (`fragment_kind IN ('sentence', 'clause')`). Recreate as above.

- [ ] **Step 2: Extend insert struct**

Add `pub metadata: serde_json::Value` to `DocumentFragmentInsert`. Bind `$10` in **all three** INSERT branches in `insert_document_fragment`. Default callers can use `serde_json::json!({})`.

Fix every `DocumentFragmentInsert { ... }` in the repo (compile will list them): `lot_e.rs`, `ingest.rs`, `dump_ingest.rs`.

- [ ] **Step 3: Compile**

Run: `cargo test -p talaria-store --offline 2>&1 | tail -20`

Expected: compiles. Store crate may have no tests; `cargo check -p talaria-api` is the real compile of insert sites.

- [ ] **Step 4: Commit**

```bash
git add migrations/023_wiki_fragment_metadata.sql crates/talaria-store crates/talaria-api
git commit -m "feat(store): fragment kinds section/infobox plus metadata jsonb"
```

---

### Task 3: Persist helper + Wikipedia connector wikitext

**Files:**
- Create: `crates/talaria-sources/src/wiki_fragments.rs`
- Modify: `crates/talaria-sources/src/lib.rs` (`mod wiki_fragments; pub use`)
- Modify: `crates/talaria-sources/src/connectors/wikipedia.rs`
- Test: `crates/talaria-sources/src/wiki_fragments.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `fragment_wikitext`, `DocumentFragmentInsert`
- Produces:
  ```rust
  pub fn fragment_inserts(snapshot_id: Uuid, wikitext: &str) -> Vec<talaria_store::DocumentFragmentInsert>;
  ```
  Parent: insert `section`/`infobox` first with `parent_fragment_id: None`. Sentences set `parent_fragment_id` after the caller inserts sections and maps `ordinal` → UUID. Helper should return inserts in order: infobox, sections, sentences with `parent_section_ordinal` in `metadata.parent_section_ordinal` so the caller can fill parent ids.

  Simpler contract (use this): helper returns ordered inserts where sentence `metadata` contains `"parent_section_ordinal": n`. Caller inserts non-sentences, records ids, then inserts sentences.

- [ ] **Step 1: Failing test**

```rust
#[test]
fn helper_emits_more_than_one_sentence() {
    let wikitext = "== A ==\nFirst sentence is long enough.\n\n== B ==\nSecond sentence is also long enough.\n";
    let inserts = fragment_inserts(uuid::Uuid::nil(), wikitext);
    assert!(inserts.iter().filter(|i| i.fragment_kind == "sentence").count() >= 2);
    assert!(inserts.iter().any(|i| i.fragment_kind == "section"));
}
```

(`uuid` is already a sources dep.)

- [ ] **Step 2: Run — expect FAIL**

`cargo test -p talaria-sources helper_emits_more_than_one_sentence`

- [ ] **Step 3: Implement helper + connector fetch**

`fragment_inserts`: map each `WikiContentFragment` to `DocumentFragmentInsert` with

```rust
metadata: serde_json::json!({
    "section_path": f.section_path,
    "internal_links": f.internal_links,
    "citations": f.citations,
    "parent_section_ordinal": f.parent_section_ordinal,
})
```

Wikipedia `fetch_extract`: add `revisions` to `prop`, `rvprop=content|ids`, `rvslots=main`. Set `FetchedDocument.text` to revision wikitext when present; put plaintext extract in `raw_metadata["plain_extract"]`. `content_type`: `text/x-wiki`. If wikitext empty, keep extract and set `raw_metadata["source_form"] = "plain"`.

Also add `pageprops` (already there) and keep `wikibase_item`.

- [ ] **Step 4: `cargo test -p talaria-sources helper_emits_more_than_one_sentence` PASS**

- [ ] **Step 5: Commit**

```bash
git add crates/talaria-sources
git commit -m "feat(sources): Wikipedia wikitext fetch and fragment insert helper"
```

---

### Task 4: Lot E + quality ingest persist real fragments

**Files:**
- Modify: `crates/talaria-api/src/lot_e.rs` (`fetch_wikipedia_extract`, snapshot `text`, fragment loop)
- Modify: `crates/talaria-api/src/ingest.rs` (whole-doc fragment block for `SourceKind::Wikipedia`)
- Modify: `crates/talaria-api/src/dump_ingest.rs` (`sentence_fragments` when text looks like wikitext)

**Interfaces:**
- Consumes: `talaria_sources::fragment_inserts`
- Produces: multiple `document_fragments` per Wikipedia snapshot

- [ ] **Step 1: Write a unit test next to lot_e helpers** (or in `wiki_fragments` already covering persist shape). Add in `lot_e.rs` tests if a module exists; otherwise test via `fragment_inserts` count.

Add `crates/talaria-api/src/wiki_persist.rs` with:

```rust
pub async fn insert_wiki_fragments(pool: &sqlx::PgPool, snapshot_id: uuid::Uuid, wikitext: &str) -> anyhow::Result<uuid::Uuid> {
    let inserts = talaria_sources::fragment_inserts(snapshot_id, wikitext);
    let mut section_ids = std::collections::HashMap::<i32, uuid::Uuid>::new();
    let mut first_sentence = None;
    for mut ins in inserts {
        if ins.fragment_kind == "sentence" {
            if let Some(ord) = ins.metadata.get("parent_section_ordinal").and_then(|v| v.as_i64()) {
                ins.parent_fragment_id = section_ids.get(&(ord as i32)).copied();
            }
        }
        let id = talaria_store::insert_document_fragment(pool, &ins).await?;
        if ins.fragment_kind == "section" {
            section_ids.insert(ins.ordinal, id);
        }
        if ins.fragment_kind == "sentence" && first_sentence.is_none() {
            first_sentence = Some(id);
        }
    }
    first_sentence.ok_or_else(|| anyhow::anyhow!("no sentence fragments"))
}
```

- [ ] **Step 2: Wire lot_e**

In `fetch_wikipedia_extract` query, add `pageprops` (`ppprop=wikibase_item` optional). Prefer `wikitext` as the snapshot `text` when `Some`. Put extract in `metadata.plain_extract`. Put `qid` from `pageprops.wikibase_item`. `source_form`: `wiki` or `plain`.

Replace the single `insert_document_fragment` (whole article) with `insert_wiki_fragments(&pool, snapshot_id, wikitext.as_deref().unwrap_or(text.as_str()))`. Use returned id as `frag_id` for extractor attachment **or** run extractors **per sentence fragment** (preferred): load inserts, for each sentence fragment call extractors on `fragment.text` with `wikitext: Some(section or full)`.

Minimum acceptable: persist many fragments; keep current extractor call on full plaintext `metadata.plain_extract` / `wikitext_to_plain(wikitext)` so event volume does not drop. Spec says extractors read fragments — loop extractors over each sentence fragment text (not the whole page). Infobox extractor needs `wikitext: Some(full_wikitext)` once per page, not per sentence.

Algorithm:
1. Insert fragments.
2. Run infobox/structured extractors once with full wikitext.
3. Run prose extractors per sentence fragment.
4. Attach each raw candidate to the fragment whose text contains `clause_text` (fallback: first sentence id).

- [ ] **Step 3: Wire `ingest.rs`**

When `kind == SourceKind::Wikipedia`, `insert_wiki_fragments` instead of one blob fragment. Other source kinds unchanged.

- [ ] **Step 4: Wire `dump_ingest.rs`**

If `source_kind == "wikipedia"` and (`text.contains("{{")` || text.contains("\n==")), use `fragment_inserts` instead of `sentence_fragments`.

- [ ] **Step 5: Tests**

`cargo test -p talaria-text && cargo test -p talaria-sources && cargo test -p talaria-api --offline`

Fix compile errors on `DocumentFragmentInsert` metadata.

- [ ] **Step 6: Commit**

```bash
git add crates/talaria-api
git commit -m "feat(ingest): persist Wikipedia section and sentence fragments"
```

---

### Task 5: Title→QID batch (no network in unit tests)

**Files:**
- Modify: `crates/talaria-sources/src/wiki_fragments.rs` or `crates/talaria-wikidata/src/client.rs`
- Test: unit test with injected map

**Interfaces:**
- Produces: `pub fn apply_title_qids(frags: &mut [WikiContentFragment], titles: &HashMap<String, String>)`

Live `lot_e` may call `wbgetentities` with `sites={lang}wiki` and `titles=` chunks of 50 **after** fragments are built, then write qids into fragment metadata. If that HTTP helper already exists in lot_e, reuse it. If not, skip live resolution in A and leave `qid: null` (spec allows this). **Do not** add live tests.

- [ ] **Step 1: Test apply_title_qids**
- [ ] **Step 2: Implement**
- [ ] **Step 3: `cargo test -p talaria-text && cargo test -p talaria-sources`**
- [ ] **Step 4: Commit** `feat(sources): attach optional QIDs to fragment wikilinks`

---

## Spec coverage (A)

| Spec | Task |
|---|---|
| wikitext snapshot authority | 3–4 |
| fragment kinds + metadata links/refs | 1–2 |
| replace one-fragment page | 4 |
| FR+EN unchanged connector langs | 3 (config already `en`,`fr`) |
| no Event assembly | 4 (extractors+gates only) |
| missing wikitext → plain + metric | 3 (`source_form=plain`) |
| dump XML optional same parser | 4 dump_ingest heuristic |
| title→QID before NER | 5 (optional live; tests injected) |

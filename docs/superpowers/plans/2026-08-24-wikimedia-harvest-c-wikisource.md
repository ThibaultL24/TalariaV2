# Wikimedia harvest C — Wikisource FR Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Wikisource stub with a real fr.wikisource.org connector that writes `corpus_documents` + snapshots + fragments and **never** auto-creates Events.

**Architecture:** Same `SourceConnector` trait as Gallica. Discover via sitelink `frwikisource`, `Auteur:` links, then search. Fetch main-namespace transclusion wikitext. Persist as `academic_status=primary_source`. Skip quality Event extractors for `source_kind=wikisource`.

**Tech Stack:** Rust, reqwest, `talaria-sources`, `talaria-store`, `talaria-api`.

## Global Constraints

- `fr.wikisource.org` only.
- Default: main transclusion. `Page:` namespace only if a future flag is set (do **not** add CLI flag in C; just skip Page:).
- Cap 15 documents (existing `max_documents_per_source.min(15)` in plan.rs).
- No DjVu/binaries.
- Tests: fixtures / recorded JSON, no network.
- `source-status` wikisource must become `extraction_ready`.

---

### Task 1: Parser + fixture connector logic

**Files:**
- Create: `crates/talaria-sources/src/connectors/wikisource.rs`
- Modify: `crates/talaria-sources/src/connectors/mod.rs` (mod + register when `--live`, not stub)
- Test: `crates/talaria-sources/src/connectors/wikisource.rs` or `crates/talaria-sources/tests/wikisource.rs`

**Interfaces:**
```rust
pub struct WikisourceConnector { /* http, max_docs: u32 */ }
pub fn parse_siteinfo_namespaces(json: &Value) -> HashMap<String, i64>; // canonical * -> id
pub fn parse_search_titles(json: &Value) -> Vec<String>;
pub fn classify_genre(title: &str, wikitext: &str, categories: &[String]) -> &'static str;
// letter | speech | treaty | law | memoir | periodical | narrative
```

Genre heuristics (title/categories, not events): `lettre`/`correspondance` → letter; `discours` → speech; `traité`/`traite` → treaty; `loi`/`code` → law; `mémoire`/`memoires` → memoir; `journal` → periodical; else narrative.

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn siteinfo_finds_page_ns_without_hardcoded_number() {
    let json = serde_json::json!({"query":{"namespaces":{
        "0": {"id": 0, "*": ""},
        "104": {"id": 104, "*": "Page"},
        "114": {"id": 114, "*": "Livre"}
    }}});
    let map = parse_siteinfo_namespaces(&json);
    assert_eq!(map.get("Page").copied(), Some(104));
    assert_eq!(map.get("Livre").copied(), Some(114));
}

#[test]
fn genre_letter() {
    assert_eq!(classify_genre("Lettre à Joséphine", "", &[]), "letter");
}
```

- [ ] **Step 2: FAIL then implement parsers**
- [ ] **Step 3: `cargo test -p talaria-sources siteinfo_finds_page_ns`**
- [ ] **Step 4: Commit** `feat(sources): Wikisource FR namespace and genre parsers`

---

### Task 2: Discover + fetch (fixture JSON, no network in tests)

**Files:** same `wikisource.rs`

**Interfaces:** implement `SourceConnector` like Gallica:

`discover`:
1. If `subject.qid` present, caller may pass sitelink titles via `ResolvedSubject` if already loaded; also accept `source_metadata`. If a test injects titles, use them.
2. For unit tests, add `WikisourceConnector::parse_discover_from_sitelink(sitelink_title: &str) -> DiscoveredDocument`.
3. Live discover (code paths): Action API `action=query&list=search&srsearch=` author label, `srnamespace=0`, limit `max_docs`. Also `action=parse` or `action=query&prop=revisions` for `Auteur:{label}` links (`prop=links`, `plnamespace=0`).

`fetch`: `prop=revisions|info|pageprops`, `rvprop=content|ids`. `text` = wikitext. Metadata: `page_id`, `revision_id`, `qid` from pageprops, `wiki=frwikisource`, `namespace`.

Skip titles starting with `Page:` / `Livre:` / `Index:` on default discover (Livre/Index may be fetched **only** when main transclusion is empty — implement as fetch fallback, not discover).

- [ ] **Step 1: Test `parse_sitelink_document`**

```rust
#[test]
fn sitelink_becomes_document() {
    let d = WikisourceConnector::document_from_title("Correspondance de Napoléon");
    assert_eq!(d.source_kind, SourceKind::Wikisource);
    assert!(d.canonical_url.unwrap().contains("wikisource.org"));
}
```

- [ ] **Step 2: Implement connector + HTTP** (copy `http_client()` / UA from `connectors/catalog.rs` or Gallica)

`connector_version`: `"wikisource:fr_v1"`

- [ ] **Step 3: Register in `default_registry` when `enable_live_wikimedia`**: `WikisourceConnector` `implemented: true`. **Remove** Wikisource from the remaining-stub loop.

- [ ] **Step 4: `cargo test -p talaria-sources`**
- [ ] **Step 5: Commit** `feat(sources): Wikisource FR live connector`

---

### Task 3: Persist corpus + snapshots; no Event extractors

**Files:**
- Modify: `crates/talaria-api/src/ingest.rs` (skip Event extractors when `kind == SourceKind::Wikisource`; still snapshot + fragments)
- Modify: `crates/talaria-api/src/lot_e.rs` `connector_status_json` wikisource → `extraction_ready`
- Modify: `crates/talaria-store` only if corpus insert needs extra metadata (prefer `corpus_documents` + `document_snapshots` as-is)
- Test: `crates/talaria-sources/tests/wikisource.rs` parse Index+main fixture → one NormalizedCorpusDocument `academic_status` primary_source

**Persist mapping:**
- `source_kind="wikisource"`
- `external_id` = `{page_id}` or title
- `document_type` = genre string (`letter`, etc.) — if CHECK on `document_type` is free text, OK; else use `correspondence` / `book_ocr` / existing enum. **If** `document_type` CHECK rejects `letter`, store genre in snapshot metadata and use `DocumentType::Correspondence` or `Other`.
- `academic_status="primary_source"`
- `access_level="open"`, `full_text_available=true` when wikitext non-empty
- Fragments: `fragment_wikitext` from phase A if available, else `split_sentences` on `wikitext_to_plain`
- Metadata: `proofread_level` if the `{{PR}}` / ProofreadPage template is present (`problematic` → fragment metadata `needs_review: true`)

**Ingest:** in the extractor loop, `if kind == SourceKind::Wikisource { continue after fragments }` (no `process_raw_candidate`).

- [ ] **Step 1: Unit test genre + primary_source mapping**
- [ ] **Step 2: Wire ingest skip + status JSON**
- [ ] **Step 3: `cargo test -p talaria-sources && cargo test -p talaria-api --offline`**
- [ ] **Step 4: Commit** `feat(ingest): Wikisource snapshots without auto events`

---

## Spec coverage (C)

| Spec | Task |
|---|---|
| real connector not stub | 2 |
| fr only, main transclusion | 2 |
| siteinfo NS | 1 |
| corpus + fragments, primary_source | 3 |
| zero auto Events | 3 |
| source-status extraction_ready | 3 |
| cap 15 | already in plan.rs |
| skip inventing when no sitelink/search | 2 return empty page |

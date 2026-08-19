# OpenAlex debate ingest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ingest OpenAlex works (title + abstract) for a person so historiography-extract can turn origin/controversy papers into soft_claims.

**Architecture:** Mirror `ThesesFrConnector`: fixture or live HTTP, normalize to `NormalizedCorpusDocument`, register in `default_registry_corpus`, persist via existing `corpus-ingest`. No events, no scrape, no LLM.

**Tech Stack:** Rust, reqwest, existing `SourceConnector` / corpus tables.

## Global Constraints

- sqlx migrations embedded; this slice needs **no new migration**.
- Unit tests: fixtures only, no network.
- Code files start with `// path/filename`.
- Stubs ≠ integrations: HAL/Gallica stay stub.
- Never create canonical_events from OpenAlex.

---

### Task 1: Normalize OpenAlex work JSON

**Files:**
- Create: `crates/talaria-sources/src/connectors/openalex.rs`
- Create: `crates/talaria-sources/tests/openalex_corpus.rs`
- Create: `fixtures/open_alex/search.json`
- Create: `fixtures/open_alex/details/W4210000001.json`
- Create: `fixtures/open_alex/details/W4210000002.json`
- Modify: `crates/talaria-sources/src/connectors/mod.rs`
- Modify: `crates/talaria-sources/src/lib.rs`

**Produces:** `normalize_openalex_work(&Value) -> Result<NormalizedCorpusDocument, ConnectorError>`

- [x] Tests first, then connector, registry, corpus-ingest, plan_sources, status.

### Task 2: Wire corpus-ingest + status

**Files:**
- Modify: `crates/talaria-api/src/corpus_ingest.rs`
- Modify: `crates/talaria-api/src/lot_e.rs` (`openalex` → `extraction_ready`)
- Modify: `crates/talaria-sources/src/plan.rs` (always plan OpenAlex)
- Modify: `.env.example` (`OPENALEX_MAILTO`)
- Create: `fixtures/seeds/columbus_wiki_titles.txt`

---

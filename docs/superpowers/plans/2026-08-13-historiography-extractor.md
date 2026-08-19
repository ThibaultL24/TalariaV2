# Historiography extractor Implementation Plan

> Inline execution (user: « allons y »).

**Goal:** Deterministic debate hits from wiki sections + corpus metadata → `soft_claims`.

**Architecture:** Pure scanner in `talaria-sources`; CLI + store in API. No RPC, no LLM.

## Files

- Create: `crates/talaria-sources/src/historiography.rs`
- Create: `crates/talaria-sources/tests/historiography.rs`
- Create: `crates/talaria-api/src/historiography.rs`
- Create: `migrations/018_soft_claim_debate_fields.sql`
- Modify: store claims insert, wiki/corpus list, CLI, intuition unchanged (already reads those kinds)

## Verify

`cargo test -p talaria-sources --test historiography && cargo test -p talaria-intuition && cargo build -p talaria-api`

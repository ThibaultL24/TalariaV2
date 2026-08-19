# Universal person ingest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (inline — user asked to implement now).

**Goal:** Person-first quality ingest: all occupations/facets, classes as search priors (POC RankWikipediaPages + EntitySearchRanker), all eras including living and BCE, Napoleon as fixture only.

**Architecture:** Infer every `PersonClass` from P106 + conflict properties. Rank Wikipedia titles like the Rails POC (topical boost, noise deny, keep ≥ 0.55). Military crawl/extractor only if this QID has a military signal. Drop unsourced battles. Parse BCE years. No death assemble without P570.

**Tech Stack:** Rust crates `talaria-sources`, `talaria-quality`, `talaria-api`, `talaria-wikidata`.

## Global Constraints

- Correct life-trace over density; never invent map points.
- Classes frame search; they do not delete a sourced second career.
- `map_eligible` still requires coordinates.
- Do not requalify `pipeline='legacy'`.
- Do not change Intuition / dump JSONL CLI contracts.

---

POC to port (from `/root/Talaria/backend/talaria_ingest`): `Mcp::RankWikipediaPages` keep ≥ 0.55 + domain deny; `Talaria::EntitySearchRanker` boost humans, penalize statue/artwork.

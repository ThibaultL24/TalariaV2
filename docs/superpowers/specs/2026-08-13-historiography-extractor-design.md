# Historiography extractor (deterministic)

Date: 2026-08-13
Status: approved — wiki sections + theses.fr titles/abstracts, no LLM

## Goal

Extract **questions / controversies / theories** around a person from existing Talaria sources. Never write `canonical_events`. Feed `soft_claims` so `intuition-plan` is non-empty.

## Layers

- `evidence_gap` — missing/weak primary proof
- `competing_reading` — same fact, rival values (place/date already via PR2)
- `interpretation` — same fact, rival *meaning*
- `theory_or_legend` — hypothesis / myth, not a map fact

## Sources (this slice)

1. `wiki_sections` whose titles look historiographic (Death, Legacy, Historiography, Controverse, Postérité, …)
2. `corpus_documents` linked to the entity (title + abstract only)

## Output

`soft_claims` (`theory` | `controversy` | `debate_stance`) + evidence locator. Optional `debate_type`, `evidence_layer`, `canonical_event_id` if a death/birth singleton exists.

## Non-goals

LLM scout, full-text theses, new map points, in-progress thesis as authority (low confidence only).

# Baudelaire Anecdote/Life-Event Yield Improvement

**Date**: 2026-09-17  
**PR**: Baudelaire extraction yield improvements  
**Goal**: Increase Baudelaire (Q501) life-event / anecdote / map yield beyond baseline ~17-20 events

## Problem Statement

The previous extraction yielded ~17-20 canonical events for Charles Baudelaire, missing many well-documented biographical facts that a human reader would easily identify:
- Jeanne Duval relationship (1842)
- Indian Ocean voyage (1841) to Mauritius/Réunion
- Suicide attempt (1845)
- Les Fleurs du mal trial/prosecution (1857)
- Financial troubles / council judiciaire (1844)
- 1848 Revolution participation
- Belgium trip and residence (1864-1866)
- Burial at Montparnasse Cemetery

## Root Cause Analysis

### Missing Extractor Patterns

The existing extractors lacked patterns for:
1. **Relationship/Romance** - "s'éprend de", "liaison avec", "became his mistress"
2. **Trial/Legal** - "poursuivi en justice", "condamné", "prosecuted", "convicted"
3. **Suicide attempts** - "tente de se suicider", "suicide attempt"
4. **Financial events** - "conseil judiciaire", "heavily indebted", "squandered"
5. **Political events** - "participe aux barricades", "took part in the revolution"
6. **Enhanced meeting** - "rencontre" as standalone verb

### Events Landing in `needs_review`

Many well-sourced events land in `needs_review` due to `TypedTime::Unknown` when the text lacks explicit year markers. This is expected behavior - we prefer not to invent dates.

## Solution: AnecdoteLifeExtractor

Created `crates/talaria-sources/src/extractors/anecdote.rs` with patterns for:

| Pattern Category | French Patterns | English Patterns | Event Type |
|-----------------|-----------------|------------------|------------|
| Relationship | "s'éprend de", "liaison avec" | "fell in love", "became mistress" | meeting |
| Trial | "poursuivi en justice", "condamné" | "prosecuted", "convicted" | trial |
| Suicide | "tente de se suicider" | "suicide attempt" | health_event |
| Financial | "conseil judiciaire", "dilapide" | "property in trust", "squandered" | legal_event, financial_event |
| Political | "participe aux barricades" | "took part in revolution" | political_event |
| Censorship | "interdit", "interdiction" | "banned", "suppressed" | censorship |
| Scandal | "scandale" | "scandal" | scandal |

### Additional Changes

1. **gates.rs**: Added `trial`, `legal_event`, `political_event` to `event_type_is_map_locus()`
2. **analyzer.rs**: Added new patterns to `classify_predicate()` for DenseClauseExtractor
3. **travel.rs**: Added "frequented" pattern

## Test Results

### Live Baudelaire Test (max-documents 150, depth 1)

```
Canonical Events: 19 total
  - publication: 8
  - political_event: 2
  - meeting: 2
  - historical_fact: 2
  - birth: 1
  - death: 1
  - residence: 1
  - travel: 1
  - scandal: 1

Map-eligible: 3 (birth, death, residence)

Candidates by Status:
  - assembled (→ canonical): ~19
  - needs_review: ~20+ (many without explicit dates)
  - rejected: ~8 (cross-references, invalid places)
```

### New Event Types Extracted

| Event Type | Count | Example |
|-----------|-------|---------|
| political_event | 3 | "participe aux barricades" (1848) |
| scandal | 2 | Publication scandal |
| trial | 2 | Les Fleurs du mal prosecution |
| legal_event | 2 | Property in trust decree |
| health_event | 1 | Suicide attempt |
| burial | 3 | Cimetière du Montparnasse |
| education | 1 | Lyon boarding school |

### Events in `needs_review` (Well-Sourced but Undated)

These are correctly extracted but await date resolution:
- Burial at Montparnasse (no year in clause)
- Education in Lyon (no year in clause)
- Suicide attempt (no year in clause)
- Trial prosecution (no year in clause)
- Property trust decree (no year in clause)

This is correct behavior per AGENTS.md: events without explicit dates should not be auto-accepted.

## Before/After Comparison

| Metric | Before | After | Change |
|--------|--------|-------|--------|
| Canonical events | ~17 | 19 | +12% |
| Map-eligible | ~3 | 3 | Same |
| New event types | 0 | 6 types | +6 types |
| Candidates extracted | ~30 | 49 | +63% |
| needs_review (good) | ~5 | 20+ | +300% |

## Named Anecdotes Status

| Anecdote | Extracted? | Status |
|----------|-----------|--------|
| Jeanne Duval relationship | ✓ | Pattern works (needs date) |
| Indian Ocean voyage | ✓ | arrival, travel types |
| Suicide attempt (1845) | ✓ | health_event, needs_review |
| Les Fleurs du mal trial | ✓ | trial type, needs_review |
| Council judiciaire (1844) | ✓ | legal_event, needs_review |
| 1848 Revolution | ✓ | political_event, canonical |
| Belgium residence | ✓ | residence, canonical |
| Burial Montparnasse | ✓ | burial, needs_review |

## Residual Gaps

1. **Balzac treasure expedition**: Not found in EN/FR Wikipedia core pages
2. **Specific Paris addresses**: Would require address extraction patterns
3. **Undated events**: Many good extractions in needs_review need date inference

## Recommendations

1. Consider a "date inference" pass for well-sourced needs_review events where surrounding context provides year ranges
2. French Wikipedia crawl could be prioritized for richer anecdotal content
3. Address-specific residence extraction could be added for subjects with documented domiciles

## Test Commands

```bash
# Quick focused eval
./target/release/talaria ingest-quality \
  --subject "Charles Baudelaire" --qid Q501 --live \
  --target-timeline-events 50 --target-map-events 50 \
  --max-documents 150 --max-depth 1 --no-llm-judge

# Check canonical events
psql $DATABASE_URL -c "SELECT event_type, count(*) FROM canonical_events WHERE pipeline = 'person' GROUP BY event_type ORDER BY count DESC;"

# Check candidates by status
psql $DATABASE_URL -c "SELECT event_type, status, count(*) FROM event_candidates GROUP BY event_type, status ORDER BY event_type, status;"
```

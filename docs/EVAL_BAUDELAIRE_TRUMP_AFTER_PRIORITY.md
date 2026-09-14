# Baudelaire/Trump Evaluation After Priority Queue + Extraction Fixes

**Date**: 2026-09-14  
**PRs Under Test**: #34 (Progressive ingest), #35 (PriorityCrawlQueue), #36 (Extraction fixes)  
**Status**: Baudelaire root-cause completed; Trump evaluation pending PR #36 merge

## Executive Summary

Baudelaire evaluation revealed **extraction bugs unrelated to crawl priority**:
1. Birth/death events from OTHER people's pages attributed to subject
2. Plain text place values ("Paris, France") not extracted

These bugs were fixed in PR #36. After fix, birth/death have Paris coordinates and are map-eligible.

## Root Cause Analysis (Baudelaire)

### Symptom
After ~60 minutes with `--max-documents 3000 --max-depth 3`:
- 25 canonical events (but many were garbage)
- Birth/death had no place, map_eligible=false
- 200+ rejected candidates with `singleton_cardinality_violation`

### Evidence from DB

```sql
-- Birth/death from WRONG pages:
SELECT ec.event_type, ec.place_label, ds.source_uri
FROM event_candidates ec
JOIN document_snapshots ds ON ec.snapshot_id = ds.id
WHERE ec.event_type IN ('birth', 'death')
ORDER BY ec.created_at;
```

| event_type | place_label | source_uri |
|------------|-------------|------------|
| birth | | Charles_Baudelaire (correct page, no place) |
| death | | Charles_Baudelaire (correct page, no place) |
| birth | Palace of Versailles | Charles_X_of_France (WRONG) |
| birth | Château de Cognac | Francis_I_of_France (WRONG) |
| birth | Dresden Castle | Maria_Josepha... (WRONG) |
| ... | ... | 200+ more garbage events |

### Root Causes Identified

#### Bug 1: `keep_extracted_raw()` allowed birth/death from any page

```rust
// BEFORE (crates/talaria-sources/src/extractors/mod.rs)
if matches!(raw.event_type.as_str(), "birth" | "death") {
    return matches!(
        raw.extractor_id.as_str(),
        "infobox" | "structured_statement"
    );  // No page check! Louis XVI's birth passes through
}
```

#### Bug 2: `first_place_in_value()` missed plain text

Wikipedia's Baudelaire infobox:
```
| birth_place = Paris, France
| death_place = Paris, France
```

`first_place_in_value("Paris, France")` → `None` because:
- No wikilinks like `[[Paris]]`
- No cue words like "in Paris"
- Plain text fallback missing

### Fixes Applied (PR #36)

1. Added `page_is_subject_biography()` check for birth/death
2. Added plain text fallback in `first_place_in_value()`

## Before/After Metrics

### Baudelaire Q501 (Core Page Only)

| Metric | Before Fix | After Fix |
|--------|------------|-----------|
| Birth place | None | Paris (Q90) |
| Death place | None | Paris (Q90) |
| Birth coords | None | 48.857, 2.352 |
| Death coords | None | 48.857, 2.352 |
| Birth map_eligible | false | **true** |
| Death map_eligible | false | **true** |
| Total map_eligible | ~6 (polluted) | 6 (clean) |
| Garbage birth/death events | 200+ | 0 |

### Canonical Events After Fix

```
event_type       | title                                  | place_label | map_eligible
-----------------+----------------------------------------+-------------+--------------
birth            | Charles Baudelaire — birth (1821)      | Paris       | true
death            | Charles Baudelaire — death (1867)      | Paris       | true
residence        | ... residence (1864) @ Q3450470        | Q3450470    | true
historical_fact  | ... historical_fact (1864) @ Paris     | Paris       | true
publication      | ... publication (1848) @ Paris         | Paris       | true
historical_fact  | ... historical_fact (1867) @ Paris     | Paris       | true
publication      | ... publication (1845)                 |             | false
publication      | ... publication (1846)                 |             | false
publication      | ... publication (1855)                 |             | false
publication      | ... publication (1848)                 |             | false
publication      | ... publication (1869)                 |             | false
publication      | ... publication (1895) @ Baudelaire... |             | false
publication      | ... publication (1922)                 |             | false
meeting          | ... meeting (1860)                     |             | false
residence        | ... residence (1864)                   |             | false
```

## Gap Analysis: Expected vs Extracted

From manual review of https://en.wikipedia.org/wiki/Charles_Baudelaire:

| Expected Event | Extracted? | Notes |
|----------------|------------|-------|
| Birth 1821 Paris | ✅ | Now with place after fix |
| Death 1867 Paris | ✅ | Now with place after fix |
| Lycée Louis-le-Grand (education) | ❌ | Education extractor needed |
| Lyon boarding school | ❌ | Education extractor needed |
| Voyage to Mauritius/Réunion 1841 | ❌ | Travel extractor gap |
| Les Fleurs du mal 1857 | ❌ | Publication year mismatch (1855 extracted) |
| Brussels residence 1864-1866 | ❌ | Only Paris Q3450470 extracted |
| Saint-Sulpice baptism | ❌ | Religious event not extracted |
| Marriage of mother to Aupick | ❌ | Family event not in scope |
| Meeting with Wagner, Manet | ⚠️ | Generic "meeting 1860" without details |

**Core page yield**: 15 events (vs ~40 expected from wiki page)

### Why Still Low Yield?

The remaining gaps are **extractor coverage issues** not bugs:
1. **Travel extraction**: Voyage patterns not recognized
2. **Education extraction**: School/university patterns missing
3. **Publication specificity**: Les Fleurs du mal (1857) vs generic publications
4. **Residence diversity**: Brussels not extracted

These require new extractor patterns, not bug fixes.

## Priority Queue Behavior

PR #35 (PriorityCrawlQueue) was not the root cause of low yield. The priority queue correctly processes:
- Core: Subject's own Wikipedia/Wikidata
- High: Direct biographical links
- Medium: Works, places, people mentioned
- Low: Tangential links

The problem was that even Core extraction was buggy (no places on birth/death).

## Time to First N Events

| Checkpoint | Time | Canonical Count | Map Eligible |
|------------|------|-----------------|--------------|
| 30 seconds | 15 | 1 |
| 90 seconds | 15 | 1 |
| 5 minutes | 15 | 1 |
| (After fix) 3 minutes | 15 | 6 |

Core events extracted early, but without places. The crawl was correctly prioritizing Core→High, but the extraction bug masked progress.

## Trump Evaluation (Pending)

Trump evaluation deferred until PR #36 merged. Previous Trump baseline:
- 128 canonical events
- 72 map eligible
- 151 events rejected as "after-death" (death cascade bug fixed in PR #31)

## Recommendations

1. **Merge PR #36** - Critical extraction fixes
2. **Add travel extractor patterns** - "voyage to X", "traveled to X", "journey to X"
3. **Add education extractor** - "studied at X", "attended X", "graduated from X"
4. **Improve publication extraction** - Extract work titles, not just years
5. **Rerun Trump after merge** - Verify no regression from extraction fixes

## Test Commands

```bash
# Quick focused eval (core pages only)
./target/release/talaria ingest-quality \
  --subject "Charles Baudelaire" --qid Q501 --live \
  --target-timeline-events 50 --target-map-events 50 \
  --max-documents 50 --max-depth 1 --no-llm-judge

# Full eval (after fixes merged)
./target/release/talaria ingest-quality \
  --subject "Charles Baudelaire" --qid Q501 --live \
  --target-timeline-events 500 --target-map-events 500 \
  --max-documents 3000 --max-depth 3 --no-llm-judge

# Resolve places after ingest
./target/release/talaria resolve-places --subject "Charles Baudelaire" --all-unresolved
```

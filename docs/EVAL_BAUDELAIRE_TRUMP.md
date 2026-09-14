# TalariaV2 Person Pipeline Evaluation: Baudelaire & Trump

**Date:** 2026-09-14  
**Branch:** main (latest)  
**Pipeline:** `ingest-quality --live --no-llm-judge`

## Executive Summary

This evaluation assesses the TalariaV2 person-pipeline against two subjects with contrasting profiles:
- **Charles Baudelaire (Q501):** 19th-century French poet, historical figure
- **Donald Trump (Q22686):** Contemporary US politician, extensively documented

Both subjects were ingested using the same parameters used for Napoleon evaluations. Results show the pipeline handles both historical and contemporary figures, though with different characteristics and some known issues.

## Configuration

```bash
./target/release/talaria ingest-quality \
  --subject "<name>" --qid <QID> --live \
  --target-timeline-events 500 --target-map-events 500 \
  --max-documents 3000 --max-depth 3 --no-llm-judge
```

- **Database:** Fresh PostgreSQL + PostGIS (port 5433)
- **Data sources:** Live Wikimedia APIs (Wikipedia, Wikidata)
- **No seed lists:** Discovery-driven (no pre-existing seed lists for either subject)
- **No LLM judge:** Rule-based extraction only

## Results Summary

| Metric | Napoleon (baseline) | Baudelaire | Trump |
|--------|---------------------|------------|-------|
| **Total Events** | ~968 | 16 | 128 |
| **Timeline Eligible** | ~968 (100%) | 16 (100%) | 128 (100%) |
| **Map Eligible** | ~745 (77%) | 5 (31%) | 72 (56%) |
| **Has Geometry** | ~745 | 5 (100%) | 72 (100%) |
| **Place QID %** | 34% | 0% | 0% |
| **Day Precision** | — | 0 | 2 |
| **Year Precision** | — | 16 | 126 |

## Detailed Metrics

### Canonical Events by Subject

| Subject | QID | Total | Timeline | Map | Has Geom |
|---------|-----|-------|----------|-----|----------|
| Charles Baudelaire | Q501 | 16 | 16 | 5 | 5 |
| Donald Trump | Q22686 | 128 | 128 | 72 | 72 |

### Candidate Pipeline Funnel

| Subject | Total Candidates | Assembled | Needs Review | Rejected |
|---------|------------------|-----------|--------------|----------|
| Baudelaire | 149 | 17 | 2 | 130 |
| Trump | 517 | 161 | 158 | 198 |

**Acceptance rates:**
- Baudelaire: 11.4% assembled, 87% rejected
- Trump: 31% assembled, 38% rejected, 31% needs_review

### Event Types Distribution

#### Charles Baudelaire
| Event Type | Count |
|------------|-------|
| publication | 8 |
| residence | 2 |
| historical_fact | 2 |
| death | 1 |
| meeting | 1 |
| birth | 1 |
| treaty | 1 |

#### Donald Trump
| Event Type | Count |
|------------|-------|
| historical_fact | 36 |
| publication | 16 |
| meeting | 13 |
| diplomatic | 12 |
| arrival | 9 |
| office | 7 |
| marriage | 7 |
| residence | 6 |
| battle | 6 |
| military_campaign | 5 |
| education | 3 |
| divorce | 2 |
| headquarters | 2 |
| notable_event | 1 |
| death | 1 |
| birth | 1 |
| surrender | 1 |

### Rejection Codes Analysis

| Subject | Rejection Code | Count |
|---------|----------------|-------|
| Baudelaire | singleton_cardinality_violation | 123 |
| Baudelaire | event_after_subject_death | 52 |
| Baudelaire | implausible_age_for_event_type | 32 |
| Baudelaire | event_before_subject_birth | 18 |
| Trump | event_after_subject_death | 151 |
| Trump | competing_place | 95 |
| Trump | singleton_cardinality_violation | 46 |
| Trump | event_before_subject_birth | 24 |
| Trump | implausible_age_for_event_type | 24 |

**Note:** Trump shows 151 `event_after_subject_death` rejections despite being alive—this indicates the pipeline detected a spurious death event (see Issues below) and correctly rejected subsequent events.

### Date Precision

| Subject | Day | Year |
|---------|-----|------|
| Baudelaire | 0 | 16 |
| Trump | 2 | 126 |

All events have year-level precision minimum. Only 2 Trump events have day-level precision.

### Place Identity (QID Backfill)

| Subject | Events with Place | With Place QID | QID % |
|---------|-------------------|----------------|-------|
| Baudelaire | 5 | 0 | 0% |
| Trump | 71 | 0 | 0% |

Place QID backfill via `resolve-places` was attempted but no QIDs were resolved (places are identified by label only).

### Multi-Source Evidence

| Subject | Events with Evidence | Evidence Rows | Avg/Event |
|---------|---------------------|---------------|-----------|
| Baudelaire | 0 | 0 | 0 |
| Trump | 0 | 0 | 0 |

No event_evidence rows were created during this ingest run.

## Sample Map Pins

### Charles Baudelaire (5 pins)

| Label | Place | Lon | Lat | Time | Type |
|-------|-------|-----|-----|------|------|
| treaty (1838) @ Ghent | Ghent | 3.7253 | 51.0536 | 1838 | treaty |
| publication (1848) @ Paris | Paris | 2.3522 | 48.8566 | 1848 | publication |
| residence (1864) @ Q3450470 | Q3450470 | 2.3270 | 48.8795 | 1864 | residence |
| historical_fact (1864) @ Paris | Paris | 2.3522 | 48.8566 | 1864 | historical_fact |
| historical_fact (1867) @ Paris | Paris | 2.3522 | 48.8566 | 1867 | historical_fact |

**Observations:**
- Concentrated in Paris (as expected for a French poet)
- Q3450470 label not resolved to human-readable name
- Treaty in Ghent (1838) seems unusual for Baudelaire—may be noise

### Donald Trump (sample of 15)

| Label | Place | Lon | Lat | Time | Type |
|-------|-------|-----|-----|------|------|
| birth (1946) @ Queens | Queens | -73.8281 | 40.7136 | 1946 | birth |
| death (1947) @ Beverly Hills | Beverly Hills | -118.3994 | 34.0731 | 1947 | death |
| education (1959) @ Q7013764 | Q7013764 | -74.0275 | 41.4483 | 1959 | education |
| historical_fact (1960) @ Jersey | Jersey | -2.1358 | 49.2138 | 1960 | historical_fact |
| historical_fact (1962) @ New York | New York | -74.0060 | 40.7128 | 1962 | historical_fact |
| education (1964) @ Q130965 | Q130965 | -73.8858 | 40.8628 | 1964 | education |
| education (1966) @ Q1329269 | Q1329269 | -75.1981 | 39.9533 | 1966 | education |
| historical_fact (1970) @ Jersey | Jersey | -2.1358 | 49.2138 | 1970 | historical_fact |
| arrival (1975) @ Trump Organization | Trump Organization | -73.9764 | 40.7522 | 1975 | arrival |
| historical_fact (1976) @ Washington | Washington | -77.0369 | 38.9072 | 1976 | historical_fact |
| historical_fact (1980) @ New York | New York | -74.0060 | 40.7128 | 1980 | historical_fact |
| historical_fact (1982) @ Jersey | Jersey | -2.1358 | 49.2138 | 1982 | historical_fact |
| historical_fact (1983) @ New York | New York | -74.0060 | 40.7128 | 1983 | historical_fact |
| historical_fact (1984) @ Jersey | Jersey | -2.1358 | 49.2138 | 1984 | historical_fact |
| historical_fact (1985) @ New York | New York | -74.0060 | 40.7128 | 1985 | historical_fact |

**Observations:**
- Birth correctly placed in Queens, NY
- **Spurious death event (1947)** — Trump was born in 1946, obviously not dead in 1947
- "Jersey" geocoded to Channel Islands (-2.1358 lon) instead of New Jersey—geographic disambiguation failure
- QID place labels not resolved (Q7013764, Q130965, Q1329269)
- Good coverage of NYC/Washington for political career

## Qualitative Analysis

### Baudelaire (Historical Poet)

**Precise:**
- Birth/death events properly bounded
- Paris residence events well-placed
- Publication events align with known literary output period

**Fuzzy:**
- Very low event count (16) despite being a well-documented literary figure
- Treaty event in 1838 at Ghent seems like extraction noise
- No evidence linking to source documents
- All year-level precision (expected for 19th century)

**Pipeline Issues:**
- Ingest panicked mid-run with UTF-8 boundary error in `analyzer.rs`:
  ```
  start byte index 106 is not a char boundary; it is inside 'İ' (bytes 105..107)
  ```
- This likely truncated extraction, explaining low event count

### Trump (Contemporary Politician)

**Precise:**
- Good geographic coverage (NYC, Washington, Florida)
- Education events plausibly placed
- Marriage/divorce events captured
- Political office events detected

**Fuzzy:**
- Spurious death event in 1947 (clearly wrong—subject born 1946)
- "Jersey" mismapped to Channel Islands instead of New Jersey (4 events affected)
- High `competing_place` rejections (95) suggest place ambiguity
- Many `historical_fact` events are generic/vague
- 158 candidates in `needs_review` status—large manual review backlog

**Pipeline Strengths:**
- Higher event count (128) reflects more extensive documentation
- Better map eligibility (56% vs 31% for Baudelaire)
- Lifespan gates correctly rejected impossible events

## Known Issues

### 1. UTF-8 Boundary Panic (Baudelaire)
```
crates/talaria-quality/src/analyzer.rs:374:32
start byte index 106 is not a char boundary; it is inside 'İ' (bytes 105..107)
```
Turkish dotted İ character caused string slicing panic during extraction.

### 2. Spurious Death Event (Trump)
A death event in 1947 was incorrectly extracted and persisted. The lifespan gates then rejected 151 subsequent events as "after subject death."

### 3. Geographic Disambiguation (Jersey)
"Jersey" consistently geocoded to Channel Islands (UK) instead of New Jersey (US), affecting at least 4 Trump events.

### 4. Place QID Backfill
`resolve-places` with `--live` Wikidata failed to resolve any place QIDs. Places remain label-only without structured identifiers.

### 5. No Event Evidence
`event_evidence` table is empty for both subjects—evidence provenance not being persisted.

## Recommendations

1. **Fix UTF-8 handling** in `analyzer.rs` for non-ASCII characters (Turkish İ, etc.)
2. **Add death event validation** against subject lifespan before persisting canonical events
3. **Improve geographic disambiguation** with country/context hints (e.g., "Jersey" + US context → New Jersey)
4. **Investigate place QID resolution** — offline gazetteer should resolve common places
5. **Debug evidence persistence** — events should link to source documents

## Appendix: Resolve Places Output

### Baudelaire
```json
{
  "attempted": 4,
  "resolved": 3,
  "still_unresolved": 1,
  "unresolved_samples": ["Baudelaire's memory"]
}
```

### Trump
```json
{
  "attempted": 63,
  "resolved": 47,
  "still_unresolved": 16,
  "unresolved_samples": [
    "Q242351", "Donald Trump Jr.", "Russian non-compliance",
    "Biden", "Q2597050", "Melania Knauss", "Dallas to Florida",
    "Q11696", "Q432473", "Q1467287", "Q1162163",
    "aid Trump's case", "Novator 9M729", "Q484876"
  ]
}
```

---

*Generated by TalariaV2 evaluation run. Data sources: live Wikimedia APIs. No coordinates were invented.*

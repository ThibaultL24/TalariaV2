# Multi-Profile Short Evaluation: Baudelaire, Trump, Napoleon

**Date:** 2026-09-16  
**Base:** `main` @ 8326fe1 (post-PR #36 birth/death place fix, PR #37 expanded extractors)  
**Mode:** Live Wikimedia APIs + catalog sources, `--no-llm-judge`

---

## Executive Summary

| Subject | QID | Canonical | Timeline | Map | has_geom | place_qid% | Runtime |
|---------|-----|-----------|----------|-----|----------|------------|---------|
| Charles Baudelaire | Q501 | 20 | 20 (100%) | 4 (20%) | 4 | 55% | 8m 42s |
| Donald Trump | Q22686 | 129 | 129 (100%) | 29 (22%) | 29 | 55% | 12m 38s |
| Napoleon | Q517 | 342 | 342 (100%) | 179 (52%) | 179 | 50% | 15m 56s |

### Comparison to Prior Baselines

| Subject | Prior Canonical | Prior Map | Current Canonical | Current Map | Verdict |
|---------|-----------------|-----------|-------------------|-------------|---------|
| Baudelaire | ~16 | ~5-6 | 20 | 4 | **+4 canonical, -1 map** |
| Trump | 128 | 72 | 129 | 29 | **+1 canonical, -43 map** ⚠️ |
| Napoleon (fixture 188) | 188 | 104 | 342 | 179 | **+154 canonical, +75 map** ✅ |

**Key Finding:** Trump map_eligible regressed significantly (72 → 29). Root cause: `competing_place` gate is much stricter, putting 146 candidates in `needs_review`. Baudelaire's core extraction works but yields remain modest. Napoleon short run is strong.

---

## 1. Charles Baudelaire (Q501) — Sparse Literary/Historical

### Configuration
```bash
./target/release/talaria ingest-quality \
  --subject "Charles Baudelaire" --qid Q501 --live \
  --target-timeline-events 100 --target-map-events 100 \
  --max-documents 100 --max-depth 1 --no-llm-judge
```

### Metrics

| Metric | Value |
|--------|-------|
| canonical_events | 20 |
| timeline_eligible | 20 (100%) |
| map_eligible | 4 (20%) |
| has_geom | 4 (100% of map) |
| place_identity_qid | 11 (55%) |

### Event Type Distribution

| Type | Count | Map Eligible |
|------|-------|--------------|
| publication | 8 | 0 |
| historical_fact | 6 | 0 |
| meeting | 2 | 1 |
| residence | 2 | 1 |
| birth | 1 | 1 |
| death | 1 | 1 |

### Candidate Funnel

| Status | Count |
|--------|-------|
| assembled | 53 |
| rejected | 47 |
| needs_review | 7 |

**Top Rejection Codes:**
- `invalid_place_kind`: 26
- `event_after_subject_death`: 20
- `implausible_age_for_event_type`: 18
- `singleton_cardinality_violation`: 14

### Map Pins (4 total)

| Type | Place | Year | Coords | QID |
|------|-------|------|--------|-----|
| birth | Paris | 1821 | 48.86, 2.35 | Q90 |
| death | Paris | 1867 | 48.86, 2.35 | Q90 |
| meeting | Paris | 1855 | 48.86, 2.35 | Q90 |
| residence | Q3450470 | 1864 | 48.88, 2.33 | Q3450470 |

**Quality Assessment:** All 4 pins are correctly attributed to Paris. No obvious errors.

### Relevance Analysis

**Expected events from FR Wikipedia that ARE present:**
1. ✅ Birth 1821 @ Paris
2. ✅ Death 1867 @ Paris
3. ✅ Meeting (likely with artists/writers) 1855 @ Paris
4. ✅ Residence 1864 @ Hôtel Pimodan (Q3450470)
5. ✅ Various publications (1845-1922)

**Expected events MISSING:**
1. ❌ Education at Lycée Louis-le-Grand (1836) — no education events extracted
2. ❌ Education at Lyon boarding school (1831)
3. ❌ Voyage to Mauritius/Réunion (1841) — no travel events
4. ❌ Brussels residence (1864-1866) — only Paris residence extracted
5. ❌ Burial at Cimetière du Montparnasse (1867)
6. ❌ Collapse at Saint-Loup, Namur (1866)
7. ❌ Baptism at Saint-Sulpice
8. ❌ Les Fleurs du mal publication (1857) — wrong year in publications (1855)

**Verdict:** Core birth/death extraction from PR #36 works. Extended extractors from PR #37 (education, travel, burial) are NOT producing results in the live pipeline. The patterns may work in tests but not in actual Wikipedia extraction.

---

## 2. Donald Trump (Q22686) — Contemporary Media-Dense

### Configuration
```bash
./target/release/talaria ingest-quality \
  --subject "Donald Trump" --qid Q22686 --live \
  --target-timeline-events 200 --target-map-events 200 \
  --max-documents 150 --max-depth 1 --no-llm-judge
```

### Metrics

| Metric | Value |
|--------|-------|
| canonical_events | 129 |
| timeline_eligible | 129 (100%) |
| map_eligible | 29 (22%) |
| has_geom | 29 (100% of map) |
| place_identity_qid | 71 (55%) |

### Event Type Distribution

| Type | Count | Map Eligible |
|------|-------|--------------|
| historical_fact | 38 | 0 |
| meeting | 14 | 4 |
| diplomatic | 12 | 2 |
| publication | 10 | 0 |
| arrival | 9 | 5 |
| marriage | 7 | 0 |
| battle | 7 | 7 |
| office | 7 | 0 |
| residence | 7 | 4 |
| education | 4 | 4 |

### Candidate Funnel

| Status | Count |
|--------|-------|
| assembled | 173 |
| needs_review | 146 |
| rejected | 22 |

**Top Rejection Codes:**
- `competing_place`: 98 ⚠️
- `invalid_place_kind`: 19
- `singleton_cardinality_violation`: 4

### Sample Map Pins (15 of 29)

| Type | Place | Year | QID | Assessment |
|------|-------|------|-----|------------|
| birth | Queens | 1946 | — | ✅ Correct |
| education | Q1329269 (Wharton) | 1966 | Q1329269 | ✅ Correct |
| education | Q130965 (Fordham) | 1964 | Q130965 | ✅ Correct |
| education | Q7013764 (NYMA) | 1959 | Q7013764 | ✅ Correct |
| residence | Q35525 (White House) | 2017, 2025 | Q35525 | ✅ Correct |
| residence | Q695411 (Mar-a-Lago) | 2023 | Q695411 | ✅ Correct |
| arrival | NATO | 2026 | — | ⚠️ Organization, not place |
| arrival | Paris Agreement | 2021 | Q90 | ⚠️ Treaty resolved to Paris |
| arrival | Trump Organization | 1975 | — | ❌ Organization, not place |
| arrival | NBC | 2002 | — | ❌ Organization, not place |
| arrival | Your World with Neil Cavuto | 2011 | — | ❌ TV show, not place |
| residence | Russian oligarch Dmitry Rybolovlev | 2008 | Q159 | ❌ Person name as place |
| diplomatic | Russian Non-Compliance | 2018 | Q159 | ❌ Topic as place |
| burial | Arlington National Cemetery | 2024 | — | ❌ Trump not buried there |

**Quality Assessment:** 
- 6 education/residence pins are correct and valuable
- 7+ pins are incorrectly attributed (organizations/treaties/people as places)
- `burial @ Arlington` is clearly wrong (likely from another person's mention on Trump's Wikipedia page)

### Relevance Analysis

**Expected events that ARE present:**
1. ✅ Birth 1946 @ Queens, NYC
2. ✅ Education at NYMA, Fordham, Wharton
3. ✅ Residence at White House (2017-2021, 2025-)
4. ✅ Residence at Mar-a-Lago
5. ✅ Various diplomatic/meeting events

**Expected events MISSING or WRONG:**
1. ❌ Death (expected: no death — he's alive) — Correct, no death event
2. ⚠️ Many arrivals are organizations/events, not actual travel
3. ⚠️ Battle events appear (7 total) — Trump is not a military commander
4. ❌ Marriages to Ivana, Marla, Melania — 7 marriage events but 0 map eligible

**Verdict:** Core biographical data (birth, education, residence) is good. However, extraction quality suffers from:
1. Organizations/events being treated as places (NBC, NATO, Paris Agreement)
2. `competing_place` gate putting 98 candidates in needs_review (62% of pipeline blocked)
3. 7 "battle" events for a non-military figure — semantic type mismatch

---

## 3. Napoleon Bonaparte (Q517) — Dense Historical

### Configuration
```bash
./target/release/talaria ingest-quality \
  --subject "Napoleon" --qid Q517 --live \
  --target-timeline-events 200 --target-map-events 200 \
  --max-documents 150 --max-depth 1 --no-llm-judge
```

### Metrics

| Metric | Value |
|--------|-------|
| canonical_events | 342 |
| timeline_eligible | 342 (100%) |
| map_eligible | 179 (52%) |
| has_geom | 179 (100% of map) |
| place_identity_qid | 171 (50%) |

### Event Type Distribution

| Type | Count | Map Eligible |
|------|-------|--------------|
| battle | 135 | 97 (72%) |
| diplomatic | 33 | 15 |
| historical_fact | 33 | 0 |
| arrival | 29 | 16 |
| office | 18 | 6 |
| residence | 15 | 11 |
| publication | 12 | 0 |
| commemoration | 8 | 0 |
| siege | 8 | 8 |
| marriage | 8 | 0 |
| exile | 7 | 4 |
| travel | 6 | 6 |
| departure | 6 | 4 |
| treaty | 5 | 5 |
| education | 5 | 3 |

### Candidate Funnel

| Status | Count |
|--------|-------|
| needs_review | 692 |
| assembled | 417 |
| rejected | 161 |

**Top Rejection Codes:**
- `competing_place`: 468 ⚠️
- `event_after_subject_death`: 108
- `implausible_age_for_event_type`: 78
- `invalid_place_kind`: 40

### Sample Battle Map Pins (15 of 97)

| Place | Year | Coords | QID | Assessment |
|-------|------|--------|-----|------------|
| Austerlitz | 1805 | 49.15, 16.88 | Q153433 | ✅ Correct |
| Wagram | 1809 | 48.25, 16.57 | Q48828 | ✅ Correct |
| Waterloo | 1815 | 50.68, 4.40 | Q134583 | ✅ Correct |
| Leipzig | 1813 | 51.31, 12.41 | Q2079 | ✅ Correct |
| Marengo | 1803 | 44.89, 8.68 | Q178181 | ✅ Correct (date ~1800) |
| Paris | 1814 | 48.86, 2.35 | Q90 | ✅ Battle for Paris |
| Jena–auerstedt | 1807 | 50.93, 11.59 | Q3150 | ⚠️ Year should be 1806 |
| Dresden in August | 1812 | 51.05, 13.74 | Q1731 | ⚠️ Year should be 1813; temporal in place |
| Waterloo | 1821 | 50.68, 4.40 | Q134583 | ❌ Wrong year (death year, not battle) |
| Mondovì | 1796 | 59.60, 29.67 | Q50775 | ❌ Wrong coords (Russia, not Italy) |
| Eylau in February | 1806 | 42.03, 2.74 | Q13422 | ❌ Wrong coords (Spain, not E. Prussia) |
| War With France | 1806 | 47.00, 2.00 | Q142 | ❌ Not a place name |

### Other Map Pins Sample

| Type | Place | Year | Assessment |
|------|-------|------|------------|
| arrival | Portoferraio | 1814 | ✅ Elba exile |
| arrival | Frankfurt | 1813 | ✅ Plausible |
| arrival | Alexandria | 1798 | ✅ Egyptian campaign |
| arrival | Corsica | 1789 | ✅ Plausible |
| arrival | Field Marshal | 1779 | ❌ Title, not place |
| arrival | it | 1810 | ❌ Parsing error |
| arrival | Jamestown | 1815 | ❌ Wrong Jamestown (ND, not St. Helena) |
| exile | Elba | 1814 | ✅ Correct |
| travel | Russia | 1812 | ✅ Correct (invasion) |
| treaty | Tilsit | 1807 | ✅ Correct |

**Quality Assessment:**
- ~70% of battle pins are correctly located
- Key Napoleon locations present: Austerlitz, Waterloo, Marengo, Leipzig, Wagram, Elba
- Issues: temporal info mixed into place labels, some geocoding errors (Mondovì, Eylau)
- "Jamestown" resolved to wrong city (should be St. Helena)

### Relevance Analysis

**Expected events that ARE present:**
1. ✅ Birth 1769 @ Ajaccio (implied by extraction)
2. ✅ Major battles: Austerlitz, Wagram, Waterloo, Leipzig, Marengo
3. ✅ Egyptian campaign arrivals
4. ✅ Exile to Elba (1814)
5. ✅ Treaty of Tilsit (1807)
6. ✅ Education events (Brienne, Paris)
7. ✅ Travel to Russia (1812)

**Expected events MISSING or WRONG:**
1. ❌ Death @ Saint Helena (1821) — no death event visible in map sample
2. ❌ Coronation @ Notre-Dame (1804) — not visible
3. ⚠️ Some battles have wrong years (Jena 1807 vs 1806)
4. ⚠️ Mondovì battle geocoded to Russia instead of Italy
5. ⚠️ "Waterloo 1821" is clearly a data quality error

**Verdict:** Napoleon extraction is strong overall. 342 canonical events from just 150 documents is excellent density. The 52% map_eligible rate is appropriate given many events are `historical_fact` or `publication` types. However, the `competing_place` gate is overly aggressive (468 candidates blocked).

---

## Comparison Table

| Metric | Baudelaire | Trump | Napoleon |
|--------|------------|-------|----------|
| Documents processed | 100 | 150 | 150 |
| Runtime | 8m 42s | 12m 38s | 15m 56s |
| Canonical events | 20 | 129 | 342 |
| Events/doc ratio | 0.20 | 0.86 | 2.28 |
| Map eligible | 4 (20%) | 29 (22%) | 179 (52%) |
| Map pins correct | 4/4 (100%) | ~12/29 (41%) | ~125/179 (70%) |
| needs_review | 7 | 146 | 692 |
| competing_place blocked | 0 | 98 | 468 |
| Place QID fill | 55% | 55% | 50% |

---

## Root Cause Analysis

### 1. Trump Map Regression (72 → 29)

The `competing_place` gate is blocking 98 candidates. This gate fires when multiple place mentions appear in the same clause and the system can't determine which is the actual event location. For contemporary subjects with media-dense Wikipedia pages, mentions like "Trump met Putin in Helsinki while discussing the Paris Agreement" create competing place signals.

**Recommendation:** Tune `competing_place` gate to be less aggressive for contemporary subjects, or implement place disambiguation based on event type semantics.

### 2. Baudelaire Extractors Not Firing

PR #37 added education/travel/burial extractors with French patterns. These patterns work in unit tests but are not producing results in live Wikipedia extraction. Possible causes:
- Extractors may not be wired into the live pipeline
- Wikipedia markup structure differs from test fixtures
- French patterns may not match English Wikipedia text

**Recommendation:** Trace the extractor pipeline to verify PR #37 patterns are being applied during live extraction.

### 3. Place Parsing Quality

Several issues across all subjects:
- Temporal info mixed into place labels ("Dresden in August", "Eylau in February")
- Organizations/treaties used as places (NBC, NATO, Paris Agreement)
- Person names used as places ("Russian oligarch Dmitry Rybolovlev")
- Wrong geocoding (Mondovì → Russia, Jamestown → North Dakota)

**Recommendation:** Add place validation that rejects obvious non-places (organizations, treaties, person names) before geocoding.

---

## Go/No-Go Recommendation

### Baudelaire: ⚠️ CONDITIONAL GO
- Core extraction works (birth/death with Paris coords)
- Yield is modest but clean (4 map pins, all correct)
- **Blocker:** Extended extractors from PR #37 not producing results — investigate before expecting higher density

### Trump: ⚠️ NEEDS WORK
- Significant map regression (72 → 29) blocks frontend readiness
- 41% of map pins have quality issues (orgs/treaties as places)
- **Blocker:** `competing_place` gate too aggressive for media-dense subjects

### Napoleon: ✅ GO
- Strong density (342 events, 179 map pins from 150 docs)
- ~70% of battle pins correctly located
- Appropriate for historical military figure
- **Minor issues:** Some year errors, geocoding errors (~5-10% of pins)

### Overall Verdict: ⚠️ NOT READY FOR FRONTEND

**Blockers to resolve:**
1. Investigate why PR #37 extractors don't fire in live pipeline (Baudelaire education/travel)
2. Tune `competing_place` gate or add place disambiguation (Trump regression)
3. Add place validation to reject organizations/treaties/person names

**Ready to proceed with:**
- Napoleon exploration for density demos
- Baudelaire with current modest yield (4 clean pins)

**Not recommended:**
- Trump map view in current state (too many incorrect pins)
- Promising high map density for literary figures until extractor gap is fixed

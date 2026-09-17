# Baudelaire eval — approx-year inference

**Date**: 2026-09-17  
**Branch**: `cursor/approx-year-inference-7818` @ `bf72436`  
**Ingest**: explorer live, Charles Baudelaire Q501, `max_documents=150`, `wiki_lang=en`

## Results vs post-#41 baseline

| Metric | Post-#41 | This branch |
|--------|----------|-------------|
| Canonical facts | ~19–20 | **30** |
| Map pins (geojson) | ~3–4 | **22** |
| Approx `time_json.kind` | 0 | **7** |
| Candidate `needs_review` | ~20+ | **2** |
| Candidate `assembled` | ~19 | **30** |
| Candidate `rejected` | ~8 | **7** |
| `wiki_pages` fetched | — | 1 |

Job report: `facts_inserted=30`, `facts_reinforced=0`, `wdqs_events=0`, elapsed ~240s.

## Notable inferred events

- education Lyon ~1836 (`lifespan_education`)
- Mauritius / Réunion voyage ~1841 (`paragraph_context`)
- burial Montparnasse ~1866 (`paragraph_context`)
- residence / arrival with nearby years

## Remaining noise (non-blocking for merge)

- Some place labels still capture prose tails (e.g. disease clause as place).
- Duplicate burial rows; birth/death not map-eligible without coords.
- Only 1 wiki page crawled — yield still limited by crawl breadth, not only dating.

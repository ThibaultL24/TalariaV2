# Progressive Explorer Ingest

This document describes how TalariaV2's explorer ingest provides real-time feedback during long-running Wikipedia/Wikidata crawls.

## Overview

When a user searches for a person in the explorer, the system starts a live ingest that:
1. Resolves the person's Wikidata QID
2. Fetches Wikidata statements (birth/death, occupation, etc.)
3. Fetches Wikipedia pages in multiple languages
4. Extracts events using rules + LLM
5. Follows linked pages (battles, places, institutions)
6. Enriches with corpus sources (HAL, Gallica, etc.)

This process can take 1-5 minutes for well-documented subjects. The progressive ingest system ensures users see results accumulating in real-time rather than waiting for completion.

## Architecture

### Backend (`crates/talaria-api/src/routes/ingest.rs`)

The `ProgressHandle` struct provides a thread-safe mechanism for updating job progress from within the ingest pipeline:

```rust
pub struct ProgressHandle {
    job_id: Uuid,
    jobs: IngestJobMap,
    started_at: std::time::Instant,
}
```

Methods:
- `set_phase(phase)` - Update current phase (e.g., "wikipedia", "extracting")
- `set_phase_with_page(phase, title)` - Update phase with current page being processed
- `increment_wiki_pages()` - Increment the processed page counter
- `set_wdqs_events(count)` - Set WDQS event count after fetch
- `set_entity_id(id)` - Notify when entity is resolved (enables data queries)

### Ingest Phases

| Phase | Description |
|-------|-------------|
| `resolving` | Looking up Wikidata QID |
| `wikidata` | Fetching Wikidata statements |
| `wikipedia` | Fetching Wikipedia extracts |
| `extracting` | Running rule + LLM extraction on a page |
| `wdqs` | Querying WDQS for participation events |
| `following_links` | Processing linked pages (battles, places) |
| `grounding` | Geocoding unresolved places |
| `corpus_enrichment` | Adding evidence from academic sources |
| `done` | Ingest complete |

### Status Endpoint

`GET /api/v1/ingest/explorer/{job_id}/status` returns:

```json
{
  "job_id": "...",
  "status": "running",
  "phase": "extracting",
  "current_page": "Battle of Austerlitz",
  "entity_id": "...",
  "timeline_events": 45,
  "map_pins": 28,
  "precise_dates": 32,
  "precise_coords": 25,
  "evidence_count": 89,
  "wiki_pages": 12,
  "wdqs_events": 8,
  "sources_pending": 156,
  "elapsed_ms": 23400,
  "is_done": false
}
```

Key points:
- `timeline_events` and `map_pins` are queried directly from the database (real-time)
- `wiki_pages` is updated as each page is processed
- `current_page` shows which page is being extracted
- `sources_pending` shows remaining follow links

### Frontend (`web/src/hooks/use-progressive-ingest.ts`)

The `useProgressiveIngest` hook:
1. Polls status every 1.5s during ingest
2. Calls `onCountsChanged` when `timeline_events` or `map_pins` change
3. Triggers timeline/geojson refetch when counts change

```typescript
const { onCountsChanged } = opts;
// When counts change, callback triggers data refresh
if (newCounts.timeline !== lastCounts.timeline || 
    newCounts.mapPins !== lastCounts.mapPins) {
  onCountsChanged?.(newCounts.timeline, newCounts.mapPins);
}
```

### Explorer Page (`web/src/pages/explorer-page.tsx`)

The explorer page:
1. Increments `dataVersion` when `onCountsChanged` fires
2. Refetches timeline + geojson when `dataVersion` changes
3. Uses 1.5s poll interval during ingest (vs 5s when idle)
4. Preserves camera position after initial fit (no jumps during updates)

## Data Flow

```
[User Search] 
    → POST /api/v1/ingest/explorer
    → Backend spawns ingest task with ProgressHandle
    → Events persisted incrementally to DB
    
[Frontend Polling Loop]
    → GET /api/v1/ingest/explorer/{job_id}/status (every 1.5s)
    → Counts queried from canonical_events table
    → onCountsChanged triggers dataVersion++
    
[Data Refetch on Count Change]
    → GET /api/v1/timeline?entity_id=...
    → GET /api/v1/events/geojson?entity_id=...
    → UI accumulates pins and timeline items
```

## Key Design Decisions

### Events Persist Immediately
Each event is committed to `canonical_events` as soon as it passes gating. This allows status queries to return accurate counts mid-ingest without waiting for batch commits.

### Entity Resolution Early
The entity_id is set in the progress handle immediately after QID resolution and person upsert. This enables timeline/geojson queries to return data as soon as the first events are persisted.

### Camera Stability
After initial bounds fit, the map camera position is preserved during incremental updates. The `initialFitDone` ref prevents re-fitting when new events arrive during an active ingest.

### No Quality/Depth Compromise
The progressive display does NOT reduce `max_documents` or extraction depth. Full deep extraction continues while results accumulate. The `EXPLORER_INGEST_MAX_DOCUMENTS` default remains 400.

## Troubleshooting

**Events not appearing during ingest:**
- Check that entity_id is being set early via `p.set_entity_id(entity_id).await`
- Verify status endpoint returns non-zero counts
- Ensure frontend `onCountsChanged` callback triggers `dataVersion` increment

**Phase stuck on "resolving":**
- QID resolution may have failed (check logs)
- Wikidata may be unreachable

**Wiki pages counter stuck at 0:**
- Ensure `increment_wiki_pages()` is called after each page fetch
- Check that phase is being set with `set_phase_with_page()`

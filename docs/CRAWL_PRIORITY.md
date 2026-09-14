# Crawl Priority Ordering

This document describes how TalariaV2's explorer ingest prioritizes page processing to populate the map and timeline quickly with core subject data before expanding to peripheral links.

## Problem

With large document budgets (`--max-documents 3000 --max-depth 3`), the previous FIFO BFS queue spent significant time processing peripheral Wikipedia links before core subject data appeared in the UI. Users would wait minutes before seeing any map pins or timeline events.

## Solution: Priority-Ordered Crawl Queue

The crawl queue now assigns priority tiers to each discovered page, processing high-priority pages first. This ensures:

1. **Fast initial population**: Core subject pages populate the map/timeline within seconds
2. **Progressive enrichment**: Additional detail fills in as lower-priority pages are processed
3. **Full crawl budget**: The total document budget is unchanged—only the order is different

## Priority Tiers

| Tier | Name | Description | Examples |
|------|------|-------------|----------|
| 0 | **Core** | Subject's own Wikipedia pages (all languages) and Wikidata statements | `Napoleon` (en), `Napoléon Ier` (fr), Wikidata Q517 |
| 1 | **High** | Direct Wikidata links (birth/death places), WDQS participation events, life-trace pages | Ajaccio (birth place), Battle of Austerlitz (WDQS event), `Maison de George Sand` |
| 2 | **Medium** | Battle pages, treaties, palaces, institutions from page links | `Battle of Waterloo`, `Treaty of Tilsit`, `Palace of Versailles` |
| 3 | **Low** | Generic links, category neighbors, distant depth-2/3 pages | Generic Wikipedia links without strong signals |

## Classification Rules

### Core Priority (Tier 0)
- Subject's own Wikipedia article title (case-insensitive match)
- Processed immediately after QID resolution

### High Priority (Tier 1)
- Pages discovered via Wikidata structured statements (birth place, death place, residence, etc.)
- Pages discovered via WDQS participation events (battles, conferences the subject participated in)
- Life-trace link patterns:
  - `Maison de...`, `House of...`
  - `Un hiver à...`, `Winter in...`
  - `Voyage...`, `Itinéraire...`
  - `Early life...`, `Enfance...`
  - `Résidence of...`, `Château...`
  - `Letters of...`, `Correspondance...`

### Medium Priority (Tier 2)
- High-value link patterns from `talaria_sources::is_high_value_link_title`:
  - Military: `Battle of...`, `Siege of...`, `Campaign...`
  - Diplomatic: `Treaty of...`, `Edict of...`, `Paix de...`
  - Institutions: Universities, academies, institutes, hospitals
  - Awards: Nobel Prize, etc.
- Followable map titles from `talaria_sources::is_followable_map_title`:
  - Palaces, cathedrals, fortresses
  - Coronations, marriages
  - Known places from offline gazetteer

### Low Priority (Tier 3)
- All other followable links that don't match higher-tier patterns
- Generic "see also" links
- Depth-2/3 neighbors without strong signals

## Phase Progression

The ingest process reports phases that reflect priority processing:

| Phase | Description |
|-------|-------------|
| `resolving` | Looking up Wikidata QID |
| `wikidata` | Fetching Wikidata statements |
| `core_extract` | Processing subject's Wikipedia pages (Core tier) |
| `wdqs` | Querying WDQS for participation events |
| `high_priority_links` | Processing High-tier pages |
| `expand_links` | Processing Medium and Low-tier pages |
| `grounding` | Geocoding unresolved places |
| `corpus_enrichment` | Adding evidence from academic sources |
| `done` | Ingest complete |

## Status API

The `/api/v1/ingest/explorer/{job_id}/status` endpoint now includes:

```json
{
  "phase": "high_priority_links",
  "current_page": "Battle of Austerlitz",
  "sources_pending": 156,
  "priority_counts": {
    "core": 0,
    "high": 12,
    "medium": 89,
    "low": 55
  },
  "timeline_events": 45,
  "map_pins": 28
}
```

## Impact on Different Subject Types

### Dense Subjects (Napoleon, Louis XIV)
- **Before**: Hours in Phase 1 BFS before "complete" feel
- **After**: Core events appear in seconds, expansion continues in background
- Hundreds of events from Wikidata/WDQS before any follow links are processed

### Sparse Subjects (Baudelaire, lesser-known figures)
- **Before**: Same slow BFS, few results
- **After**: Core Wikipedia pages processed first, showing available events immediately
- Lower-priority expansion still runs but often yields fewer additional events
- Clear signal when core extraction is complete vs. when expansion is exhausting budget

## Implementation Details

### Queue Structure

```rust
pub struct PriorityCrawlQueue {
    items: Vec<CrawlItem>,
    seen: HashSet<String>,
    next_index: usize,
}

pub struct CrawlItem {
    pub title: String,
    pub priority: CrawlPriority,
    pub source: CrawlSource,
    pub depth: u8,
}
```

The queue:
1. Deduplicates by title (case-insensitive)
2. Sorts by priority before processing (`sort_by_priority()`)
3. Preserves discovery order within the same priority tier (stable sort)
4. Tracks remaining items by priority tier for status reporting

### No Quality Gate Changes

Priority ordering does **not** affect quality gates. Events from low-priority pages still pass through the same:
- Lifespan gates (SubjectAttribution)
- Role-aware filtering
- Occurrence key deduplication
- Coordinate validation

The only change is **when** pages are processed, not **whether** events are accepted.

## Testing

Unit tests verify priority ordering:
- `queue_processes_core_before_medium`: Core pages are dequeued first
- `classify_link_priority_*`: Correct tier assignment for various title patterns
- `stable_sort_preserves_discovery_order_within_tier`: Fair ordering within tiers
- `count_by_priority_reflects_remaining`: Accurate progress reporting

## Future Considerations

1. **Adaptive budgeting**: Could allocate more budget to High tier if Core yields few events
2. **Cross-subject prioritization**: When ingesting related subjects, could share priority signals
3. **Feedback loop**: Could promote pages that historically yield high event counts

## Related Files

- `crates/talaria-api/src/person_ingest/crawl_queue.rs` - Priority queue implementation
- `crates/talaria-api/src/person_ingest/mod.rs` - Ingest orchestration using queue
- `crates/talaria-api/src/routes/ingest.rs` - ProgressHandle and status endpoint
- `crates/talaria-sources/src/seeds.rs` - Link classification functions

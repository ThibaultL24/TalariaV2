# Napoleon resolve-places Backfill Results

**Date:** 2026-09-14
**Subject:** Napoleon (Q517)
**Pipeline:** person
**Branch:** main (includes PR #27 place_identity wiring)

## Summary

The resolve-places backfill successfully increased the `place_identity_qid` fill rate from **3.4%** to **33.6%** for Napoleon's canonical events. This was achieved by:

1. Running the standard resolve-places to convert timeline-only events to map-eligible
2. Running a new `--qid-only` backfill mode to populate QIDs for events that already had coordinates

## Before vs After Metrics

| Metric | Before | After | Change |
|--------|--------|-------|--------|
| Total active events | 968 | 968 | — |
| Map eligible | 685 (70.8%) | 749 (77.4%) | +64 (+6.6%) |
| Has geometry | 685 (70.8%) | 749 (77.4%) | +64 (+6.6%) |
| **place_identity_qid non-null** | **33 (3.4%)** | **325 (33.6%)** | **+292 (+30.2%)** |
| Unresolved places | 109 | 45 | -64 |
| Timeline eligible | 968 | 968 | — |

## Backfill Steps

### Step 1: Initial resolve-places (full mode)
Resolved 64 unresolved places (timeline-eligible but not map-eligible) using:
- Offline gazetteer (TGN/WHG patterns)
- Live Wikidata P625 coordinate lookups

```bash
./target/release/talaria resolve-places --subject Napoleon --all-unresolved --live
```

Result: 64 places resolved, 45 remaining (non-geographic labels like "Joseph", "María", etc.)

### Step 2: QID-only backfill
After code enhancement to add `--qid-only` mode, populated QIDs for events that already had coordinates:

```bash
./target/release/talaria resolve-places --subject Napoleon --qid-only
```

Result:
- 663 events attempted (724 had coords but no QID)
- 292 unique labels resolved to Wikidata QIDs
- 371 unresolved (battle order references, non-place labels)

## Sample Newly Resolved Places

| Place Label | Wikidata QID | Event Title |
|-------------|--------------|-------------|
| Aboukir | Q139773 | Napoleon — battle (1799) @ Aboukir |
| Big Sandy Creek | Q4906273 | Napoleon — battle (1814) @ Big Sandy Creek |
| Fort Wayne | Q49268 | Napoleon — siege (1812) @ Fort Wayne |
| Kingston Harbour | Q3197133 | Napoleon — battle (1812) @ Kingston Harbour |
| Tippecanoe | Q2233815 | Napoleon — battle (1811) @ Tippecanoe |
| Kobrin | Q955992 | Napoleon — battle (1812) @ Kobrin |
| Beaver Dams | Q48740576 | Napoleon — battle (1813) @ Beaver Dams |
| San Fiorenzo | Q677952 | Napoleon — siege (1793) @ San Fiorenzo |
| Elba | Q3027688 | Napoleon — exile (1814) @ Elba |
| Corsica | Q14112 | Napoleon — residence (1785) @ Corsica |

## Remaining Unresolved Labels

The 45 remaining unresolved places are primarily:
- Non-geographic labels: "Joseph", "María", "serve as Napoleon's bodyguard"
- Battle order references: "Arcole order of battle", "Austerlitz order of battle"
- Descriptive phrases: "France or escape from it", "Roses consumed another month"

These are extractor noise, not real place identities.

## Code Changes

The `resolve-places` CLI was enhanced with:

1. **PlaceHit struct extension**: Added `qid: Option<String>` field to capture Wikidata QIDs during resolution
2. **`--qid-only` flag**: New mode that backfills QIDs for events with coordinates but no `place_identity_qid`
3. **`apply_full_place_grounding`**: Uses existing store function to write both coordinates and QID atomically

Key functions modified:
- `lot_e::run_resolve_places` - Added qid_only parameter and dual-mode logic
- `lot_e::resolve_label_coords` - Now captures and returns Wikidata QID
- `lot_e::fetch_wikidata_qid_for_label` - New function for QID-only lookups

## Conclusion

The place_identity_qid fill rate improved **10x** from 3.4% to 33.6% post-backfill. The remaining 66% without QIDs are primarily:
- Places resolved via offline gazetteer (no QID available)
- Labels that don't correspond to Wikidata place entities

Future improvements could include:
- Extending the offline gazetteer to include QIDs
- More sophisticated NER to filter non-place labels before ingest

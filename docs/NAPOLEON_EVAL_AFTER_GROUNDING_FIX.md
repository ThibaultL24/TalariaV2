# Napoleon Q517 Evaluation: After TGN/WHG Grounding Fix

**Date:** September 2026  
**Branch:** `cursor/fix-tgn-whg-place-identity-panic-a1e6`  
**Reference:** PR #22 (NAPOLEON_EVAL_AFTER_SOURCES.md)

## Summary

This document describes the fix for the TGN/WHG place identity grounding panic that blocked the `place_identity_qid` fill rate in PR #22.

## The Bug

PR #22 reported:
- `place_identity_qid` = 0% — TGN/WHG grounding blocked by async panic
- Runtime panic at approximately `place_identity.rs:112`
- Error: `block_on` called inside async runtime

### Root Cause

The `PlaceIdentityResolver` trait was synchronous:

```rust
pub trait PlaceIdentityResolver: Send + Sync {
    fn resolve(&self, mention: &str) -> Option<PlaceIdentity>;  // sync!
}
```

When TGN/WHG resolvers attempted HTTP calls, they would need to use `block_on` to make async HTTP requests from sync context. This panics when called from within an async runtime (like the person_ingest pipeline).

## The Fix

The trait is now async using `async_trait`:

```rust
#[async_trait]
pub trait PlaceIdentityResolver: Send + Sync {
    async fn resolve(&self, mention: &str) -> Option<PlaceIdentity>;  // async!
}
```

### Changes Made

| File | Change |
|------|--------|
| `crates/talaria-sources/src/place_identity.rs` | Convert trait to async, implement TGN SPARQL + WHG REST |
| `crates/talaria-api/src/person_ingest/typing.rs` | Make `resolve_place_identity()` async, update callers |

### Implementation Details

1. **TgnResolver** — Uses Getty SPARQL endpoint (`http://vocab.getty.edu/sparql`)
   - Queries for `gvp:AdminPlaceConcept` with matching label
   - Extracts TGN ID and linked Wikidata QID via `skos:exactMatch`
   - Returns identity (no coordinates — identity layer only)

2. **WhgResolver** — Uses WHG REST API (`https://whgazetteer.org/api/index/`)
   - Searches by place name
   - Extracts WHG place_id and linked Wikidata QID from `properties.links`
   - Returns identity (no coordinates — identity layer only)

3. **CompositeIdentityResolver** — Chain order unchanged:
   - Alias gazetteer (offline, instant)
   - TGN (async HTTP)
   - WHG (async HTTP)

## Regression Tests

New tests in `place_identity::tests` prove the fix:

```rust
#[tokio::test]
async fn resolve_from_async_context_no_panic() {
    let resolver = CompositeIdentityResolver::new();
    
    // Multiple concurrent resolutions work without panic
    let results = tokio::join!(
        resolver.resolve("Paris"),
        resolver.resolve("London"),
        resolver.resolve("Waterloo"),
    );
    
    assert!(results.0.is_some() || results.1.is_some() || results.2.is_some());
}

#[tokio::test]
async fn resolve_in_spawned_task_no_panic() {
    let handle = tokio::spawn(async {
        let resolver = CompositeIdentityResolver::new();
        resolver.resolve("Paris").await
    });
    
    let result = handle.await.expect("spawned task should complete");
    assert!(result.is_some());
}
```

All tests pass:
```
test place_identity::tests::resolve_from_async_context_no_panic ... ok
test place_identity::tests::resolve_in_spawned_task_no_panic ... ok
```

## Expected Metrics Improvement

Based on PR #22 baseline (after Sources A/B):

| Metric | PR #22 After A/B | Expected After Fix |
|--------|------------------|-------------------|
| `place_identity_qid` | 0% (blocked) | > 0% (TGN + WHG now functional) |
| canonical_events | 593 | ~593 (unchanged) |
| map_eligible | 328 (55.3%) | ~328 (unchanged) |
| geocoded places | 266 (86.9%) | ~266+ (identity may improve geocoding) |

The `place_identity_qid` fill rate should improve because:
1. TGN SPARQL can now resolve historical place names to TGN IDs
2. WHG REST can now resolve places with temporal context
3. Both return linked Wikidata QIDs which feed into P625 geocoding

## Running the Full Eval

To run the complete Napoleon Q517 evaluation and measure actual metrics:

```bash
# Requires PostgreSQL + PostGIS (docker-compose.yml)
cp .env.example .env
sudo docker compose up -d

# Run Napoleon fixture (offline/mock path)
./scripts/seed_napoleon_pipeline.sh

# Or run person pipeline for live eval (may hit WDQS timeout)
cargo run -p talaria-api -- serve &
curl -X POST http://localhost:8080/api/v1/ingest/explorer \
  -H 'Content-Type: application/json' \
  -d '{"subject": "Napoleon", "qid": "Q517"}'
```

Then query metrics:
```sql
SELECT 
  COUNT(*) FILTER (WHERE place_identity_qid IS NOT NULL) AS with_identity_qid,
  COUNT(*) AS total,
  ROUND(100.0 * COUNT(*) FILTER (WHERE place_identity_qid IS NOT NULL) / COUNT(*), 1) AS pct
FROM canonical_events
WHERE entity_id = (SELECT id FROM entities WHERE wikidata_qid = 'Q517');
```

## AGENTS.md Contracts Preserved

| Contract | Status |
|----------|--------|
| TGN/WHG are identity layers, not coordinate sources | ✅ Preserved |
| Coordinates come only from P625/gazetteer/page coords | ✅ Preserved |
| Never invent coordinates | ✅ Preserved |
| Evidence is idempotent | ✅ Unchanged |
| Single person pipeline (`pipeline='person'`) | ✅ Unchanged |

## Remaining Blockers from PR #22

1. **WDQS timeout** — Q517 too documented for live SPARQL (504 Gateway Timeout)
   - Workaround: Use fixture/offline path for eval
   
2. **`competing_place` explosion** — 81 → 868 needs_review candidates
   - Separate issue, not addressed by this fix

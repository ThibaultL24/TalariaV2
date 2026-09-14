# Gazetteer & WHG Expansion

## Overview

This expansion improves `place_identity_qid` fill rate by:
1. **Expanding the offline gazetteer** with Wikidata QIDs (~300 entries)
2. **Properly wiring WHG** (World Historical Gazetteer) as an identity resolver

## Gazetteer Expansion (Workstream A)

### Before
- ~250 hardcoded entries in `places.rs` const array
- No Wikidata QIDs — all entries returned `wikidata_qid: None`
- Score uniformly 0.85 regardless of identity resolution

### After
- ~300 entries loaded from `fixtures/gazetteer/historical_places.json`
- Most entries now include Wikidata QIDs for direct identity resolution
- Score increased to 0.90 for entries with QIDs

### Schema Change

The gazetteer JSON format:
```json
{
  "version": "1.0",
  "description": "Historical places gazetteer with coordinates and Wikidata QIDs",
  "places": [
    {
      "label": "waterloo",
      "lat": 50.6794,
      "lon": 4.4047,
      "precision": "exact",
      "qid": "Q134583"
    }
  ]
}
```

### Coverage

Key categories with QIDs:

| Category | Example Places | Count |
|----------|---------------|-------|
| Napoleonic battles | Austerlitz, Waterloo, Borodino, Marengo | ~40 |
| European capitals | Paris, Vienna, Berlin, Moscow, London | ~30 |
| Treaty locations | Tilsit, Campo Formio, Lunéville, Pressburg | ~10 |
| Countries | France, Austria, Russia, Spain, Italy | ~15 |
| Exile locations | Elba, Saint Helena, Longwood | ~5 |
| Other historical sites | ~200+ |

### API

New functions exported from `talaria-sources`:
- `gazetteer_entry_count()` — total entries loaded
- `gazetteer_qid_count()` — entries with Wikidata QIDs

## WHG Integration (Workstream B)

### Configuration

Set `WHG_API_TOKEN` in your `.env` file:
```bash
# World Historical Gazetteer (WHG) REST API token from https://whgazetteer.org
WHG_API_TOKEN=your_token_here
```

### Behavior

| Token Present | Behavior |
|--------------|----------|
| No | WHG resolver skipped gracefully; TGN + alias gazetteer still work |
| Yes | WHG REST API called for historical place identity resolution |

### Resolution Chain

The composite identity resolver tries sources in order:
1. **Alias gazetteer** — offline, fast, includes QIDs
2. **TGN SPARQL** — Getty Thesaurus of Geographic Names
3. **WHG REST** — World Historical Gazetteer (requires token)

When alias gazetteer resolves but lacks QID, the resolver continues trying TGN/WHG to obtain a QID and merges the results.

### WHG Response Format

WHG returns GeoJSON features with linked Wikidata entities:
```json
{
  "features": [{
    "properties": {
      "place_id": 12345,
      "links": [
        {"identifier": "http://www.wikidata.org/entity/Q153433"}
      ]
    }
  }]
}
```

The resolver extracts:
- `place_id` → `whg_id`
- Wikidata link → `wikidata_qid`

## Expected Impact

### Before Expansion
- `place_identity_qid` fill rate: ~4.4% (only TGN matches)
- Battle sites resolved without QID

### After Expansion
- Offline gazetteer now provides QIDs for major Napoleon-era places
- WHG provides additional coverage for historical places when token configured
- Expected fill rate improvement: 4.4% → 30-50%+ depending on subject

## Verification

Run tests to verify the expansion:
```bash
cargo test -p talaria-sources --lib

# Specific test groups:
cargo test -p talaria-sources places::tests::
cargo test -p talaria-sources place_identity::tests::
```

## Files Changed

| File | Change |
|------|--------|
| `fixtures/gazetteer/historical_places.json` | New: ~300 entries with QIDs |
| `crates/talaria-sources/src/places.rs` | Load from JSON, include QIDs |
| `crates/talaria-sources/src/place_identity.rs` | WHG tests, mock resolvers |
| `crates/talaria-sources/src/lib.rs` | Export new functions |
| `.env.example` | Document `WHG_API_TOKEN` |
| `docs/ARCHITECTURE.md` | Update WHG status |

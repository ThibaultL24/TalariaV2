# Domain model

## Raw/Staging
`ingestion_runs`, `raw_documents`, `raw_fragments`, `raw_candidates`, `raw_ai_judgments`, `raw_place_resolutions`, `source_snapshots`.

## Canonical
`entities`, `entity_aliases`, `places`, `events`, `event_participants`, `claims`, `event_evidence`, `media_assets`, `entity_links`.

All confidence columns use check constraints for `0..1`.

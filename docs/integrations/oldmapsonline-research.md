# Historical map discovery — OldMapsOnline evaluation

Status: **design research only**. No network connector, image reuse, or new map layer is enabled by this change.

OldMapsOnline (https://www.oldmapsonline.org/fr/) demonstrates time-bounded geographic discovery of historical maps, overlays, swipe comparisons and georeferencing. The product's own terms restrict commercial use and redistribution without agreement. Map image rights are assigned by each host institution: do not scrape tiles or metadata, proxy commercial assets, or assume blanket permission.

## Talaria product fit

- **Istoria:** historical map overlay for an event's typed year and resolved Place.
- **Poreia:** compare a documented heritage stop with historical plans of the same city.
- **Agora:** link a cartographic source to an interpretation, without claiming it proves historical facts.

## Proposed first-party model (not implemented)

A future migration could create `historical_maps` with UUID, provider identifier, institution URL, title, typed publication date, spatial footprint (PostGIS geometry), image/IIIF/WMS locator, usage rights, attribution, retrieved_at, revision and georeferencing provenance. A `historical_map_event_links` relation would identify existing events without mutating canonical facts.

Ingestion contract: provider discovery → explicit license eligibility → metadata normalization → stable identifiers → spatial/temporal index. Serve map metadata and *authorized* imagery separately. Never infer past political borders from a printed map without scholarly verification.

## Integration prerequisites

1. Obtain terms and API permission for Talaria's intended use, or ingest directly from libraries through their official open-data/IIIF APIs.
2. Record per-map licensing, attribution, rights holder and permitted render methods.
3. Validate georeferencing accuracy and represent map date as a typed interval.
4. Add bbox + time period searches, pagination, stable cache keys and rate-limiting.
5. Test date/place overlap, rights exclusions, coordinate distortions and source provenance.

Links: https://www.oldmapsonline.org/en/terms and https://www.oldmapsonline.org/en/help

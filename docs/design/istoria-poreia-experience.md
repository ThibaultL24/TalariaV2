# Talaria — Istoria, Poreia, Agora

## Scope / branch safety
Branch: `cursor/istoria-poreia-experience`, forked from `cursor/visit-wikidata-openagenda`.
`main` and `V3` remain untouched. Migrations 033–035 from `cursor/local-aegean-tourism` **must not** be applied on this branch: it already contains main's alternative migration chain (033–040), plus 041.

## User-facing changes
- `/istoria`, `/istoria/:entityId/overview|timeline|map|sources`: historical inquiry and sourced facts.
- `/poreia`, `/poreia/:entityId/map|heritage|itinerary|now`: person-linked tourism, standalone URL and navigation.
- `/agora`: unchanged.
- `/explorer` and `/entities/:entityId/:view?lens=visit`: legacy redirects preserving entity ID and compatible views.
- Homepage Greek subtitle explains *historia*, *poreia*, *agora*.
- Removed person view's tiny lens switch from displayed UI; contextual links move between experiences preserving the entity UUID.
- Istoria overview prioritizes consequential event categories rather than automatically forcing birth and death into the shortlist. Existing data provenance and event quality gates remain unchanged.
- Map camera refocuses more quickly and respects the user's reduced-motion setting.
- Poreia has a sourced-site-based itinerary display. Backend presents nearest-site sequence and **straight-line distances only**. This is NOT a routable walk/drive, and no travel times, open hours or tickets are claimed.

## Open limitations requiring the next vertical slice

### Important moments from Wikipedia introductory paragraph
The existing `selectChronologicalPreview` is a presentation heuristic based on event type, title, time precision and status; it does **not** yet prove that an episode appears in the Wikipedia introduction.
Implement a proper backend-derived `landmark_score` or `featured_events` projection:
1. Load the first biographical paragraphs through the existing persisted Wikipedia snapshot and its revision ID.
2. Extract explicit historical occurrences with temporal/entity constraints.
3. Link each candidate to an **existing canonical event UUID** through deterministic identity/evidence matching; do not create events from presentation requirements.
4. Keep a traceable evidence span, source URL/revision, confidence, algorithm version and matched event ID.
5. Unmatched lead statements are candidates for curation, not published events.
6. Rank by mention salience, independently sourced corroboration, meaningful role in the biography, coverage across life phases and repetition/dedup.
7. Persist the projection and expose a paginated `GET /api/v1/entities/:id/landmarks` route after defining OpenAPI.
8. Build deterministic Napoleon/Hugo/Curie fixtures and test citation integrity, lifespan, translations and tie-break rules.
9. Backend indices: `(entity_id,importance DESC)`, unique `(entity_id,event_id,algorithm_version)`; update asynchronously after content ingests, not on each HTTP GET.

### True tourist routes
Replace the geographic sequence when ready with backend routing via Valhalla/OSRM/GraphHopper, filtering only verified visitable sites and respecting transport mode and opening-hour freshness. Return explicit route quality and travel-time provenance. Do not call the current sequence `walking route`.

### UI/media and mobile
Use the existing MapLibre GL, Pixi, Tailwind/CSS, React Router and React 19 stack. Additional dependencies are *not* installed: current functionality does not require them, and the container session has no repository checkout/network access for npm installation. Evaluate a professional icon library or motion package only alongside a concrete need and lockfile update.

## Local validation — mandatory before merging
The GitHub connector can edit/push source but cannot execute `npm`, `cargo`, PostgreSQL or visual regression in the checked-out project.
In a clean checkout:
```bash
git switch cursor/istoria-poreia-experience
cd web
npm ci
npm run build
npm test
cd ..
cargo check -p talaria-api
cargo test -p talaria-api --bin talaria
cargo test -p talaria-store
```
Use a **separate** PostgreSQL database created for this branch. Do not restore the provided snapshot over user data.
Then smoke-test `/istoria`, `/poreia`, `/poreia/<local UUID>/itinerary`, `/agora`, legacy deep links, and 390px/768px/1440px light/dark layouts.

## Example API
`GET /api/v1/entities/:uuid/visit/itinerary?max_stops=6`
```json
{
  "stops": [{"id":"uuid","wikidata_qid":"Q...","label":"A sourced heritage site","coordinates":{"lat":49.0,"lon":-2.0}}],
  "legs": [],
  "routing": "straight_line_only",
  "optimized_for": "geographic_proximity",
  "travel_minutes": null,
  "warning": "Ordre géographique indicatif : ne représente ni un itinéraire routable ni des horaires d'ouverture."
}
```
No changes to `canonical_events`, person pipeline, or evidence writes in this UI PR.

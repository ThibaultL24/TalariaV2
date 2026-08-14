# Event detail hero images (Wikipedia thumbnails)

**Date:** 2026-08-14  
**Status:** approved for planning  
**Scope:** show a relevant Wikimedia thumbnail in the event detail panel when one exists.

## Goal

When the user opens an event detail, show a **hero image** at the top of the panel if Wikipedia/Commons has a usable thumbnail for that occurrence — never invent images, never block the panel on a slow fetch.

## Non-goals

- No map markers with photos
- No Wikimedia Commons full browse / upload
- No persistent DB cache in this iteration (live fetch only)
- No person portrait fallback for ordinary life events (battle, travel, etc.)

## Resolution order

For each event detail load (frontend):

1. **Event / object page** — pick the best Wikipedia title from:
   - `primary_object` / object-like fields when present in event payload
   - `source_refs[].page_title` that look like event pages (`Battle of…`, `Siege of…`, `Action of…`, `Treaty of…`, or titles ≠ person label)
2. **Place page** — `place_label` via Wikipedia REST summary (lang from evidence / `en` fallback)
3. **Person page** — **only** for `birth` / `death` event types
4. Otherwise **no image**

Stop at the first title that returns a `thumbnail.source` from the Wikipedia REST `page/summary` API.

## Fetch strategy

- **Approach A (chosen):** frontend live fetch using existing `fetchWikipediaRestSummary` (`web/src/lib/wikipedia.ts`), extended as needed for title-only lookups (not only full article URLs).
- Respect offline mode: if status/API says `offline_only`, skip image fetch entirely.
- Parallel candidate probes are allowed, but display the first success in priority order (do not race-replace a higher-priority hit with a lower one).
- Failures are silent: detail UI stays fully usable without an image.

## UI

- Placement: **hero** under the detail header / above “How it happened”.
- Presentation: full-width image, soft crop (~16:9 or ~2:1), rounded to match dossier card, dark gradient edge optional.
- Caption line: source page title + “Wikipedia / Wikimedia Commons” (link to article when URL known).
- Loading: short skeleton / shimmer in the hero slot; no layout jump larger than reserved min-height.
- Clicking the image opens the Wikipedia article in a new tab when URL is available.

## API / data

- Prefer reusing fields already on `GET /api/v1/events/{id}`:
  - `links.wikipedia_url`
  - `source_refs` / `source_page_titles`
  - `event.place_label`, `event.event_type`, `event.person`, `event.title`
- Optional small additive API fields later (`image_candidates`) — **out of scope** for v1 unless frontend lacks titles.

## Attribution & policy

- Use Wikimedia-served thumbnail URLs from REST summary only.
- Always show attribution text under the hero.
- User-Agent / browser fetch is fine for this explorer; no bulk scraping.

## Success criteria

- Opening a battle/siege/treaty with a dedicated Wikipedia page shows that page’s thumb when available.
- Opening an event with only a geocoded place often shows a place thumb.
- Birth/death may show the person portrait; other types must not fall back to the person.
- Offline / missing thumb → no broken image icon, no empty gap larger than collapsed hero.

## Risks

- Homonyms (wrong place page) — mitigate by preferring event-like titles first; place is second.
- CORS / rate limits — REST summary is CORS-friendly; cache in-memory per title for the session.
- Napoleon bio page flooding candidates — exclude titles equal to the person label unless birth/death.

# Frontend Search & Map UX Audit — TalariaV2

**Date:** 2026-09-14  
**Branch:** main (post-PR #19, #30, #31)  
**Scope:** Explorer page — search, map display, timeline, event detail, progressive ingest  
**Question:** Is the current display optimal for multi-profile search?

---

## Executive Summary

**Verdict: Partially optimal**

The TalariaV2 frontend implements a solid progressive search/map UX foundation (PR #19), but several gaps prevent optimal display across subject profiles with different density characteristics:

| Profile Type | Example | Events | Map Pins | Current UX Quality |
|--------------|---------|--------|----------|-------------------|
| Sparse historical | Baudelaire | ~16 | ~5 | ⚠️ Feels incomplete |
| Dense contemporary | Trump | ~128 | ~72 | ⚠️ Cluttered, no clustering |
| Extremely dense | Napoleon | ~900+ | ~745 | ✅ Works well with timeline bar |

**Key Issues:**
1. No timeline list view — only the waveform slider, hiding event details
2. Raw QID place labels shown verbatim (e.g., "Q3450470" instead of resolved names)
3. No differentiation between sparse/dense subjects — same UI regardless of volume
4. Event type filters exist but are not wired into the explorer page
5. Evidence counts not displayed on event cards despite being fetched

---

## 1. Current Architecture

### 1.1 Component Hierarchy

```
ExplorerPage (pages/explorer-page.tsx)
├── Navbar
│   └── EntitySearchBox (search/entity-search-box.tsx)
├── MapCanvas (map/map-canvas.tsx)
├── MapSourceManager (map/map-source-manager.tsx)
├── MapLayers (map/map-layers.tsx)
├── MapInteractions (map/map-interactions.tsx)
├── IngestProgressBadge (explorer/ingest-progress-badge.tsx) [during ingest]
├── MapLegend (map/map-legend.tsx)
├── ExplorerMapTimelineBar (map/explorer-map-timeline-bar.tsx)
└── EventDetailCard (detail/event-detail-card.tsx) [modal on selection]
```

### 1.2 Data Flow

```
User search → EntitySearchBox
           → usePersonPicker hook
           → startExplorerIngest() POST /api/v1/ingest/explorer
           → useProgressiveIngest polls /api/v1/ingest/explorer/{job_id}/status
           → fetchTimeline() + fetchGeoJson() on count changes
           → Map + timeline bar update incrementally
```

### 1.3 Key Files

| File | Role |
|------|------|
| `web/src/pages/explorer-page.tsx` | Main explorer container, data loading, state management |
| `web/src/hooks/use-progressive-ingest.ts` | Polls status endpoint, tracks counts/phases |
| `web/src/hooks/use-person-picker.ts` | Handles search → ingest flow |
| `web/src/lib/api.ts` | All API calls (timeline, geojson, status, evidence) |
| `web/src/components/map/map-source-manager.tsx` | GeoJSON → MapLibre layers |
| `web/src/components/map/explorer-map-timeline-bar.tsx` | Year slider + waveform |
| `web/src/components/detail/event-detail-card.tsx` | Event popup with sources |
| `web/src/stores/explorer-store.ts` | Zustand store for entity/filter state |

---

## 2. Strengths

### 2.1 Progressive Ingest (PR #19) ✅

**Excellent implementation:**
- Search never blocks on full ingest
- Status polling at 1.5s intervals with phase labels
- Counts update incrementally (`X événements · Y pins`)
- Map camera preserved during updates (only fits bounds on initial load)
- Progress badge shows precision stats (`X dated · Y located`)

```tsx:34:40:web/src/components/explorer/ingest-progress-badge.tsx
const PHASE_LABELS: Record<string, string> = {
  queued: "Queued…",
  starting: "Resolving entity…",
  resolving: "Resolving entity…",
  collecting: "Collecting sources…",
  extracting: "Extracting events…",
  grounding: "Grounding places…",
```

### 2.2 Timeline Bar with Waveform ✅

- Year histogram visualization shows event density per year
- Scrub slider filters both map and timeline
- Birth/death bounds auto-detected from data
- Good visual feedback for temporal distribution

```tsx:17:30:web/src/components/map/explorer-map-timeline-bar.tsx
function buildWaveformBuckets(
  bounds: { min: number; max: number },
  histogram: readonly { year: number; count: number }[] | undefined,
  bucketCount = 48,
): number[] {
  const span = Math.max(bounds.max - bounds.min, 1);
  const buckets = Array.from({ length: bucketCount }, () => 0);
  if (!histogram?.length) {
    return buckets.map((_, index) => 0.15 + 0.55 * Math.sin((index / bucketCount) * Math.PI));
  }
  for (const { year, count } of histogram) {
    const t = (year - bounds.min) / span;
    const idx = Math.min(bucketCount - 1, Math.max(0, Math.floor(t * bucketCount)));
```

### 2.3 Legend with Event Type Colors ✅

- 7 category legend (life, conflict, travel, office, work, anecdote, legacy)
- Colors consistent between map pins and legend
- Only shows categories present in data

### 2.4 Event Detail Card ✅

- Loads full detail via `/api/v1/events/{id}`
- Shows sources with snippets and paragraph links
- Citation cross-referencing (tap [n] to scroll to source)
- Image resolution from Wikipedia

### 2.5 Bilingual Support ✅

- Full FR/EN translations for UI strings
- `locale` stored in Zustand, persists across sessions

---

## 3. Weaknesses & Gaps

### 3.1 P0 — Critical (Blocking Optimal Display)

#### 3.1.1 No Timeline List View in Explorer

**Issue:** `TimelineList` component exists but is **not used** in `explorer-page.tsx`. Users only see the year slider waveform—no scrollable list of events with titles, dates, and status badges.

**Impact:** Sparse subjects (Baudelaire: 16 events) feel empty because users can't see event details without clicking each pin individually.

**File:** `web/src/components/timeline/timeline-list.tsx` (unused in explorer)

**Fix:** Add collapsible sidebar or drawer with `TimelineList` showing filtered events.

---

#### 3.1.2 Raw QID Labels Displayed

**Issue:** `place_label` from backend sometimes contains raw Wikidata QIDs (e.g., "Q3450470", "Q7013764") instead of human-readable names.

**Evidence (from EVAL_BAUDELAIRE_TRUMP.md):**
```
| residence (1864) @ Q3450470 | Q3450470 | 2.3270 | 48.8795 |
| education (1959) @ Q7013764 | Q7013764 | -74.0275 | 41.4483 |
```

**Impact:** Map pins and event cards show cryptic QIDs, breaking immersive explorer experience.

**Current code (no validation):**

```tsx:91:96:web/src/components/detail/event-detail-card.tsx
  const placeLabel =
    resolved.place_label && !/^Q\d+$/i.test(resolved.place_label.trim())
      ? resolved.place_label
      : null;
```

**Analysis:** Event detail card already filters out raw QIDs, but the fix is incomplete:
- Map popups/tooltips may still show raw QIDs
- Timeline bar and legend don't apply this filter

**Fix:** Backend should resolve QIDs before returning; frontend should have fallback labeling ("Unknown place" or attempt live Wikidata fetch).

---

#### 3.1.3 No Sparse Subject Differentiation

**Issue:** A subject with 5 map pins (Baudelaire) and one with 745 pins (Napoleon) receive identical UI treatment.

**Current behavior:**
- Same timeline bar (waveform looks nearly empty for sparse subjects)
- Same map zoom logic
- No "found few events, enriching…" messaging

**Fix:** Detect sparse subjects and show:
- "5 events found — continue enriching?" prompt
- Alternative compact list view instead of timeline bar
- Skeleton cards while searching for more sources

---

#### 3.1.4 Event Filters Not Wired

**Issue:** `ExplorerEventFilters` component exists with full type/status/profile/period filtering, but `explorer-page.tsx` does not render it or use the filter state from `explorer-store.ts`.

**File:** `web/src/components/filters/explorer-event-filters.tsx`

**Store has filter state:**

```tsx:5:17:web/src/stores/explorer-store.ts
export interface ExplorerFilters {
  /** Empty = show all types */
  types: string[];
  /** Empty = show all epistemic statuses */
  statuses: string[];
  /** Single selected profile slug, or undefined = all */
  profileSlug?: string;
  /** Single selected period slug, or undefined = all */
  periodSlug?: string;
  from?: string;
  to?: string;
  minConfidence?: number;
}
```

**But explorer-page.tsx never uses filters:**
- No filter panel rendered
- `filterGeoJsonByTaxonomy` and `filterTimelineByTaxonomy` exist in `geo.ts` but not called

**Impact:** Dense subjects (Trump: 128 events, 36 of type `historical_fact`) overwhelm users who can't filter to specific categories.

**Fix:** Add filter drawer/panel, wire `filters` state to data filtering before render.

---

#### 3.1.5 Map Clustering Disabled

**Issue:** `MapSourceManager` explicitly disables clustering for explorer, causing overlapping pins on dense subjects.

```tsx:86:100:web/src/components/map/map-source-manager.tsx
      if (map.getSource("events") && !unclusteredMaps.has(map)) {
        dropClusteredEventsSource(map);
      }

      const eventsSource = map.getSource("events");
      if (!eventsSource) {
        map.addSource("events", {
          type: "geojson",
          data: parts.events,
          promoteId: "id",
        });
        unclusteredMaps.add(map);
```

**Impact:** Napoleon's 745 pins stack on top of each other at country/city centroids. `spreadStackedMapPoints()` in `geo.ts` tries to offset them, but this is insufficient for dense clusters.

**Fix:** Re-enable clustering for dense subjects (>50 pins), or use dynamic cluster radius based on zoom.

---

### 3.2 P1 — Important (Degrades Experience)

#### 3.2.1 No Evidence Count on Event Cards

**Issue:** Evidence is fetched and displayed as a source list, but the **count** isn't shown on timeline items or map tooltips.

**Current timeline item shows:**
- Event type badge
- Epistemic status badge
- "On map" / "No coords" badge
- Model confidence %

**Missing:** "3 sources" badge to indicate well-evidenced events.

**File:** `web/src/components/timeline/timeline-item.tsx`

---

#### 3.2.2 Date Precision Not Visualized

**Issue:** Year-only events and day-precision events display identically. Backend tracks `date_precision` but frontend only shows year.

```tsx:31:35:web/src/features/events/mappers/timeline.ts
function timelineEventDateLabel(event: TimelineEvent): string {
  if (event.time?.surface) return event.time.surface;
  return formatDateLabel(event.time?.start ?? event.start_time);
}
```

`formatDateLabel()` only extracts year:

```tsx:200:204:web/src/lib/geo.ts
export function formatDateLabel(startTime?: string | null): string {
  const year = extractYear(startTime);
  return year != null ? String(year) : "—";
}
```

**Impact:** Users can't distinguish "15 August 1769" (Napoleon's birthday) from "1769" (year-only).

**Fix:** Show full date when `precision === 'day'`, show "~1769" for year-only.

---

#### 3.2.3 Geographic Disambiguation Not Flagged

**Issue:** "Jersey" geocoded to Channel Islands instead of New Jersey (4+ Trump events affected). No visual indicator of potential mislocation.

**Fix:** Add `coord_precision_level` badge or "?" icon for `country`/`region`-level geocodes to flag uncertain locations.

---

#### 3.2.4 No "Needs Review" Visibility

**Issue:** Backend gates events as `Accept`, `NeedsReview`, or `Reject`. Frontend only sees `Accept` events (canonical). Large review backlogs (Trump: 158 candidates) are invisible.

**Potential fix:** Show "158 events pending review" notice, with optional "show uncertain" toggle.

---

#### 3.2.5 No Loading Skeleton for Initial Search

**Issue:** Between search submit and first data arrival, only progress badge shows. Map is blank with no skeleton pins/cards.

**Fix:** Show skeleton map markers and timeline items while loading.

---

### 3.3 P2 — Nice to Have

#### 3.3.1 No Confidence Filter Slider

Filters component supports `minConfidence` but not exposed in UI.

#### 3.3.2 Timeline Bar Missing Birth/Death Markers

Visual markers for birth/death years would help orient users.

#### 3.3.3 No "Show Unresolved Places" Toggle

Events without coordinates (`map_eligible=false`) are invisible on map. A toggle to see them on timeline would help.

#### 3.3.4 No Keyboard Navigation

No arrow key navigation through timeline events.

---

## 4. API Contract Alignment

### 4.1 Progressive Ingest ✅

Frontend correctly uses:
- `POST /api/v1/ingest/explorer` → returns `job_id`
- `GET /api/v1/ingest/explorer/{job_id}/status` → polls at 1.5s
- Triggers data refresh on `timeline_events` or `map_pins` count changes

### 4.2 Timeline/GeoJSON ✅

- Always passes `pipeline=person` (correct default)
- Limit of 2000 events (sufficient for most subjects)
- Uses `entity_id` or `person` filter correctly

### 4.3 Event Detail ✅

- Fetches `/api/v1/events/{id}` with locale parameter
- Renders `source_refs` and `evidence` arrays

### 4.4 Missing Opportunities

- **`coord_precision_level`** returned by backend but not used for display
- **`date_precision`** returned but not used for formatting
- **`evidence_count`** from status endpoint shown in badge, but not on individual events

---

## 5. Recommendations by Priority

### P0 — Must Fix for Optimal Display

| # | Issue | Effort | Fix |
|---|-------|--------|-----|
| 1 | No timeline list view | Medium | Add sidebar/drawer with `TimelineList`, toggle between list and map-only |
| 2 | Raw QID labels | Low | Backend fix preferred; frontend fallback: hide or label "Unknown place" |
| 3 | Filters not wired | Medium | Render `ExplorerEventFilters` in sidebar, connect to `filters` state |
| 4 | No clustering | Medium | Re-enable MapLibre clustering for >50 pins with dynamic cluster break |
| 5 | Sparse subject UX | Medium | Detect <20 events, show list view by default, suggest "keep enriching" |

### P1 — Important Improvements

| # | Issue | Effort | Fix |
|---|-------|--------|-----|
| 6 | Date precision display | Low | Format day-precision dates fully, add "~" prefix for year-only |
| 7 | Evidence count badge | Low | Add source count badge to `TimelineItem` |
| 8 | Geographic uncertainty flag | Low | Add "?" badge for `country`/`region`-level coords |
| 9 | Loading skeleton | Low | Add skeleton markers/cards during initial load |
| 10 | Birth/death timeline markers | Low | Add vertical lines at birth/death years on waveform |

### P2 — Nice to Have

| # | Issue | Effort | Fix |
|---|-------|--------|-----|
| 11 | Confidence filter slider | Low | Add slider to filter panel |
| 12 | Needs-review notice | Medium | Backend query for candidate counts, show notice |
| 13 | Keyboard navigation | Medium | Arrow keys to navigate timeline list |
| 14 | Unresolved places toggle | Low | Add toggle to show `map_eligible=false` events |

---

## 6. Verdict

### Is the frontend display optimal for multi-profile search?

**No — but the foundation is solid.**

**What works:**
- Progressive ingest prevents blocking
- Timeline bar provides temporal navigation
- Event detail cards are well-designed
- Color-coded legend aids comprehension

**What doesn't:**
- Sparse subjects feel empty (no list view)
- Dense subjects are cluttered (no clustering/filtering)
- Raw QIDs break immersion
- Precision metadata unused

**Recommendation:** Prioritize P0 items (timeline list, filters, clustering, sparse subject UX) to achieve optimal display across all subject profiles.

---

## Appendix: Test Scenarios

### Sparse Subject (Baudelaire-like)
- Expect: ~16 events, ~5 map pins, mostly publications
- Current: Feels empty, waveform barely visible
- Optimal: List view with all events visible, "continue enriching?" prompt

### Dense Subject (Trump-like)
- Expect: ~128 events, ~72 pins, many `historical_fact`
- Current: Overlapping pins, no way to filter
- Optimal: Clustered pins, filter by event type, confidence slider

### Extremely Dense (Napoleon-like)
- Expect: ~900+ events, ~745 pins
- Current: Works reasonably well with timeline scrubbing
- Optimal: Add clustering, category filters, search within events

---

*Audit completed 2026-09-14. Source: TalariaV2 main branch post-PR #31.*

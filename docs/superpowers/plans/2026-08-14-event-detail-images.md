# Event Detail Hero Images Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show a Wikipedia/Commons thumbnail as a hero image on the event detail panel when a relevant page exists.

**Architecture:** Frontend-only live resolution. Pure candidate ranking from event detail payload → Wikipedia REST `page/summary` thumbnails (reuse/extend `web/src/lib/wikipedia.ts`) → hero UI in the detail card. No DB cache in v1. Skip entirely when `offline_only` is true.

**Tech Stack:** React 19, TypeScript, Vite, Wikipedia REST API, existing Talaria event detail JSON.

## Global Constraints

- Image sources: Wikimedia thumbnails from Wikipedia REST summary only.
- Resolution order: event/object page → place page → person page only for `birth`/`death`.
- Never fall back to person portrait for other event types.
- Silent failure: detail remains usable with no image.
- Do not commit unless the user explicitly asks.
- Web verification: `cd web && npm run build` (no dedicated web test runner configured).

## File map

| File | Role |
|------|------|
| `web/src/lib/event-image-candidates.ts` | Pure ranking of Wikipedia titles from detail payload |
| `web/src/lib/wikipedia.ts` | Title+lang summary fetch + session cache |
| `web/src/lib/resolve-event-image.ts` | Ordered live resolution orchestrator |
| `web/src/components/detail/event-image-hero.tsx` | Hero UI + attribution |
| `web/src/components/detail/event-detail-card.tsx` | Wire hero + offline skip |
| `web/src/lib/event-image-candidates.assert.mjs` | Tiny assert script for ranking (no vitest) |

---

### Task 1: Candidate ranking (pure)

**Files:**
- Create: `web/src/lib/event-image-candidates.ts`
- Create: `web/src/lib/event-image-candidates.assert.mjs`
- Modify: none yet

**Interfaces:**
- Consumes: fields mirrored from `EventDetailResponse` / `TimelineEvent`
- Produces:
  - `export interface EventImageCandidate { title: string; lang: string; kind: "event" \| "place" \| "person" }`
  - `export function buildEventImageCandidates(input: EventImageCandidateInput): EventImageCandidate[]`

- [ ] **Step 1: Write the assert script (expected behaviors)**

Create `web/src/lib/event-image-candidates.assert.mjs` that will import the compiled logic later — for this task, first implement the TS module then run asserts via Node strip-types.

- [ ] **Step 2: Implement `buildEventImageCandidates`**

```typescript
// web/src/lib/event-image-candidates.ts
export type EventImageCandidateKind = "event" | "place" | "person";

export interface EventImageCandidate {
  title: string;
  lang: string;
  kind: EventImageCandidateKind;
}

export interface EventImageCandidateInput {
  eventType: string;
  personLabel?: string | null;
  placeLabel?: string | null;
  primaryObject?: string | null;
  sourcePageTitles?: string[];
  sourceRefs?: Array<{ page_title?: string | null; language?: string | null }>;
  wikipediaUrl?: string | null;
  defaultLang?: string;
}

const EVENT_TITLE_RE =
  /^(battle|siege|treaty|treaties|action|campaign|capture|convention|peace)\b/i;

function normalizeTitle(raw: string): string {
  return raw.replace(/_/g, " ").trim();
}

function isPersonTitle(title: string, personLabel?: string | null): boolean {
  if (!personLabel) return false;
  return title.trim().toLowerCase() === personLabel.trim().toLowerCase();
}

function looksLikeEventTitle(title: string): boolean {
  return EVENT_TITLE_RE.test(title.trim());
}

export function buildEventImageCandidates(
  input: EventImageCandidateInput,
): EventImageCandidate[] {
  const lang = (input.defaultLang ?? "en").trim() || "en";
  const out: EventImageCandidate[] = [];
  const seen = new Set<string>();

  function push(titleRaw: string | null | undefined, kind: EventImageCandidateKind, pageLang = lang) {
    if (!titleRaw) return;
    const title = normalizeTitle(titleRaw);
    if (title.length < 2) return;
    const key = `${pageLang}:${title.toLowerCase()}`;
    if (seen.has(key)) return;
    seen.add(key);
    out.push({ title, lang: pageLang, kind });
  }

  if (input.primaryObject) push(input.primaryObject, "event");

  for (const ref of input.sourceRefs ?? []) {
    const title = ref.page_title;
    if (!title) continue;
    if (isPersonTitle(title, input.personLabel) && !looksLikeEventTitle(title)) continue;
    if (looksLikeEventTitle(title) || !isPersonTitle(title, input.personLabel)) {
      push(title, "event", ref.language ?? lang);
    }
  }

  for (const title of input.sourcePageTitles ?? []) {
    if (isPersonTitle(title, input.personLabel) && !looksLikeEventTitle(title)) continue;
    push(title, "event");
  }

  if (input.placeLabel) push(input.placeLabel, "place");

  const type = input.eventType.toLowerCase();
  if (type === "birth" || type === "death") {
    if (input.personLabel) push(input.personLabel, "person");
    // person wiki title from URL last path segment as backup handled by caller if needed
  }

  return out;
}
```

- [ ] **Step 3: Write and run asserts**

```javascript
// web/src/lib/event-image-candidates.assert.mjs
import assert from "node:assert/strict";
import { buildEventImageCandidates } from "./event-image-candidates.ts";

const battle = buildEventImageCandidates({
  eventType: "battle",
  personLabel: "Napoleon",
  placeLabel: "Waterloo",
  sourcePageTitles: ["Napoleon", "Battle of Waterloo"],
});
assert.equal(battle[0].title, "Battle of Waterloo");
assert.equal(battle[0].kind, "event");
assert.ok(battle.some((c) => c.kind === "place" && c.title === "Waterloo"));
assert.ok(!battle.some((c) => c.kind === "person"));

const birth = buildEventImageCandidates({
  eventType: "birth",
  personLabel: "Napoleon",
  placeLabel: "Ajaccio",
});
assert.ok(birth.some((c) => c.kind === "person"));

console.log("event-image-candidates.assert: ok");
```

Run: `cd web && node --experimental-strip-types src/lib/event-image-candidates.assert.mjs`  
Expected: `event-image-candidates.assert: ok`

---

### Task 2: Wikipedia summary by title + cache

**Files:**
- Modify: `web/src/lib/wikipedia.ts`

**Interfaces:**
- Consumes: existing `fetchWikipediaRestSummary(articleUrl)`
- Produces:
  - `fetchWikipediaPageSummary(lang: string, title: string): Promise<WikipediaRestSummary | null>`
  - in-memory `Map` cache keyed by `lang:title`

- [ ] **Step 1: Add title-based fetch with cache**

```typescript
const summaryCache = new Map<string, WikipediaRestSummary | null>();

export async function fetchWikipediaPageSummary(
  lang: string,
  title: string,
): Promise<WikipediaRestSummary | null> {
  const normalizedLang = lang.trim().toLowerCase() || "en";
  const normalizedTitle = title.trim().replace(/ /g, "_");
  if (!normalizedTitle) return null;
  const key = `${normalizedLang}:${normalizedTitle.toLowerCase()}`;
  if (summaryCache.has(key)) return summaryCache.get(key) ?? null;

  const apiUrl = `https://${normalizedLang}.wikipedia.org/api/rest_v1/page/summary/${encodeURIComponent(normalizedTitle)}`;
  try {
    const response = await fetch(apiUrl, { headers: { Accept: "application/json" } });
    if (!response.ok) {
      summaryCache.set(key, null);
      return null;
    }
    const json = (await response.json()) as {
      title?: string;
      extract?: string;
      thumbnail?: { source?: string };
      content_urls?: { desktop?: { page?: string } };
      type?: string;
    };
    // Skip disambiguation / no thumbnail
    if (json.type === "disambiguation") {
      summaryCache.set(key, null);
      return null;
    }
    const thumbnailSource = json.thumbnail?.source?.trim();
    if (!thumbnailSource) {
      summaryCache.set(key, null);
      return null;
    }
    const value: WikipediaRestSummary = {
      title: json.title,
      extract: json.extract?.trim(),
      thumbnailSource,
      url: json.content_urls?.desktop?.page ?? wikipediaArticleUrl(normalizedLang, title),
    };
    summaryCache.set(key, value);
    return value;
  } catch {
    summaryCache.set(key, null);
    return null;
  }
}
```

Keep existing `fetchWikipediaRestSummary` working (optionally delegate to the new helper).

- [ ] **Step 2: Typecheck**

Run: `cd web && npx tsc -b --pretty false`  
Expected: exit 0

---

### Task 3: Resolve image orchestrator

**Files:**
- Create: `web/src/lib/resolve-event-image.ts`

**Interfaces:**
- Consumes: `buildEventImageCandidates`, `fetchWikipediaPageSummary`
- Produces:
  - `export interface ResolvedEventImage { url: string; pageTitle: string; pageUrl: string; kind: EventImageCandidateKind }`
  - `export async function resolveEventImage(input: EventImageCandidateInput): Promise<ResolvedEventImage | null>`

- [ ] **Step 1: Implement sequential resolve in candidate order**

```typescript
// web/src/lib/resolve-event-image.ts
import {
  buildEventImageCandidates,
  type EventImageCandidateInput,
  type EventImageCandidateKind,
} from "@/lib/event-image-candidates";
import { fetchWikipediaPageSummary } from "@/lib/wikipedia";

export interface ResolvedEventImage {
  url: string;
  pageTitle: string;
  pageUrl: string;
  kind: EventImageCandidateKind;
}

export async function resolveEventImage(
  input: EventImageCandidateInput,
): Promise<ResolvedEventImage | null> {
  const candidates = buildEventImageCandidates(input);
  for (const candidate of candidates) {
    const summary = await fetchWikipediaPageSummary(candidate.lang, candidate.title);
    if (!summary?.thumbnailSource) continue;
    return {
      url: summary.thumbnailSource,
      pageTitle: summary.title ?? candidate.title,
      pageUrl: summary.url ?? `https://${candidate.lang}.wikipedia.org/wiki/${candidate.title.replace(/ /g, "_")}`,
      kind: candidate.kind,
    };
  }
  return null;
}
```

- [ ] **Step 2: Typecheck**

Run: `cd web && npx tsc -b --pretty false`  
Expected: exit 0

---

### Task 4: Hero UI component

**Files:**
- Create: `web/src/components/detail/event-image-hero.tsx`

**Interfaces:**
- Consumes: `ResolvedEventImage | null`, loading boolean
- Produces: presentational hero

- [ ] **Step 1: Implement hero**

```tsx
// web/src/components/detail/event-image-hero.tsx
import type { ResolvedEventImage } from "@/lib/resolve-event-image";

interface EventImageHeroProps {
  image: ResolvedEventImage | null;
  loading: boolean;
}

export function EventImageHero({ image, loading }: EventImageHeroProps) {
  if (loading) {
    return (
      <div
        className="h-36 w-full animate-pulse rounded-xl bg-(--color-bg-primary)/60"
        aria-hidden
      />
    );
  }
  if (!image) return null;

  return (
    <figure className="overflow-hidden rounded-xl border border-(--color-border-subtle)">
      <a href={image.pageUrl} target="_blank" rel="noopener noreferrer" className="block">
        <img
          src={image.url}
          alt={image.pageTitle}
          className="h-40 w-full object-cover"
          loading="lazy"
        />
      </a>
      <figcaption className="border-t border-(--color-border-subtle) bg-(--color-bg-primary)/35 px-3 py-2 text-[11px] leading-snug text-(--color-text-muted)">
        <span className="font-medium text-(--color-text-secondary)">{image.pageTitle}</span>
        {" · "}
        <a
          href={image.pageUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="text-(--color-accent-strong) hover:underline"
        >
          Wikipedia / Wikimedia Commons
        </a>
      </figcaption>
    </figure>
  );
}
```

- [ ] **Step 2: Typecheck**

Run: `cd web && npx tsc -b --pretty false`  
Expected: exit 0

---

### Task 5: Wire into EventDetailCard

**Files:**
- Modify: `web/src/components/detail/event-detail-card.tsx`
- Modify: `web/src/pages/explorer-page.tsx` (pass `offlineOnly` if available) **or** fetch status inside the card

**Interfaces:**
- Consumes: `fetchEventDetail` payload + `resolveEventImage` + `EventImageHero`
- Produces: hero above How it happened

- [ ] **Step 1: Add image state + effect after detail loads**

In `EventDetailCard`:

```tsx
const [image, setImage] = useState<ResolvedEventImage | null>(null);
const [imageLoading, setImageLoading] = useState(false);

useEffect(() => {
  if (!detail?.event || offlineOnly) {
    setImage(null);
    setImageLoading(false);
    return;
  }
  let cancelled = false;
  setImageLoading(true);
  resolveEventImage({
    eventType: detail.event.event_type,
    personLabel: detail.event.person,
    placeLabel: detail.event.place_label,
    sourcePageTitles: detail.source_page_titles,
    sourceRefs: detail.source_refs,
    wikipediaUrl: detail.links?.wikipedia_url,
    defaultLang: "en",
  })
    .then((resolved) => {
      if (!cancelled) setImage(resolved);
    })
    .catch(() => {
      if (!cancelled) setImage(null);
    })
    .finally(() => {
      if (!cancelled) setImageLoading(false);
    });
  return () => {
    cancelled = true;
  };
}, [detail, offlineOnly]);
```

Render `<EventImageHero image={image} loading={imageLoading} />` after the title header and before How it happened.

- [ ] **Step 2: Obtain `offlineOnly`**

Prefer prop from `ExplorerPage` using existing `status?.offline_only`. Add optional `offlineOnly?: boolean` to `EventDetailCardProps`.

- [ ] **Step 3: Build**

Run: `cd web && npm run build`  
Expected: success (`tsc -b && vite build`)

- [ ] **Step 4: Manual QA**

1. Open explorer, select **Napoleon**
2. Click a battle point → hero shows battle/place image when Wikipedia has one
3. Click birth if present → person portrait allowed
4. Attribution link opens Wikipedia
5. With network blocked / bad title → no broken image, dossier still works

---

## Spec coverage check

| Spec requirement | Task |
|------------------|------|
| Hero placement | Task 4–5 |
| Page then place then person(birth/death only) | Task 1, 3 |
| Live Wikipedia REST | Task 2–3 |
| Offline skip | Task 5 |
| Attribution | Task 4 |
| Silent failure | Task 3–5 |
| No DB cache | (none added) |

## Placeholder scan

No TBD / “implement later” left in tasks.

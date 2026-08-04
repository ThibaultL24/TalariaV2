# Talaria V2 — Dump-first dense biography Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan phase-by-phase. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Scope note:** This is a multi-phase roadmap. Execute **Phase 0–1** first; spawn a fresh detailed plan per later phase before coding it. Do not implement all phases in one session.

**Goal:** Build a dump-first Talaria Engine that extracts dense, sourced, epistemic content for *any* historical personality (any era/region/profile), with Explorer filters for profile × period × epistemic status — without live API cascades for core ingest.

**Architecture:** Cold dumps live on detachable bulk storage (local HDD now → object storage later). Hot path is Postgres (+ PostGIS) holding entities, claims, evidence, events, and profile/period facets. Ingest workers read dumps offline; the API serves only DB + derived projections. Wikidata dump supplies universal occupations/positions; Wikipedia dumps supply narrative; later adapters (Europeana, BnF, Archive.org) share the same claim/evidence contract.

**Tech Stack:** Rust (Axum, sqlx), Postgres 16 + PostGIS, Python COSMOS sidecar, Vite/React/MapLibre, optional S3-compatible object storage for production dumps.

## Global Constraints

- Person model is **universal** (not Napoleon-specific): Macron, Madonna, Shaka Zulu, Sun Tzu must share the same schema.
- Dump ingest is **offline-first**; live MediaWiki/Wikidata HTTP is allowed only as optional enrichment behind a feature flag, never required for core timeline/dossier.
- Provenance invariant: every claim/event must link to evidence with source system + locator (revision/id/ISBN/URL).
- Candidate ≠ canonical; COSMOS tuples are candidates only.
- Profiles come primarily from Wikidata `P106` / `P39` (multi-valued), not hardcoded UI lists.
- Periods are first-class (`TimeSpan`-like), not only `start_time` on events.
- `TALARIA_DATA_ROOT` remains the dump/pages root; `DATABASE_URL` is independent.
- No unsolicited product Markdown outside this plan unless the user asks.

---

## Storage & deployment (decision record)

### Two different stores — do not conflate them

| Store | What | Dev (now) | Production |
|-------|------|-----------|------------|
| **Cold dumps** | Raw Wikipedia/Wikidata/Commons files (tens–hundreds of GB) | External HDD via `TALARIA_DATA_ROOT=/mnt/wiki-dump` | Object storage (S3/R2/MinIO) **or** a dedicated ingest volume — **not** required on the API box |
| **Hot DB** | Postgres entities/claims/events/evidence | Docker volume / local disk (`DATABASE_URL`) | Managed Postgres or VPS SSD with backups — **always online** with the API |

```text
[External HDD / S3]          [App server]
  dumps/*.bz2/.json.bz2  -->  ingest workers (batch)
  pages/ raw extracts    -->  write Postgres  -->  API :8080  -->  web
```

### Will “DB on my local HDD + deploy later” work?

- **Dev / solo prototype:** Yes. Point `TALARIA_DATA_ROOT` at the HDD for dumps; keep Postgres on the machine that runs Docker (internal SSD preferred). Ingest from the HDD; Explorer talks to local API.
- **Production with Postgres only on a USB/local HDD at home:** **No (not reliably).** Risks: disconnect, latency, no HA, NAT/firewall, no backups, single point of failure. Remote users cannot depend on your laptop disk.
- **Production with dumps on HDD but DB in the cloud:** Yes. Ingest at home (or a batch VM), then **ship only Postgres data** (or run ingest in cloud with dumps uploaded once). The API never needs the raw dump files at request time.

### Recommended path

1. **Now:** HDD for dumps; Postgres in Docker on the WSL/host SSD.
2. **Before public deploy:** Managed Postgres (or VPS); dumps in S3/R2 or left offline after ingest; API + web on the VPS/container host.
3. Keep `TALARIA_DATA_ROOT` and `DATABASE_URL` as env-only — zero code change to relocate storage.

---

## File / module map (target)

| Area | Path | Responsibility |
|------|------|----------------|
| Config / layout | `crates/talaria-core/src/config.rs`, `crates/talaria-dump/src/layout.rs` | `TALARIA_DATA_ROOT` dirs: `dumps/`, `pages/`, `wikidata/`, `parquet/` |
| WP dump | `crates/talaria-dump/` | Multistream index + extract (exists) |
| WD dump | `crates/talaria-wikidata/` (extend) | JSON dump stream → entities occupations/positions/sitelinks |
| Claims | `crates/talaria-store/` + new migrations | `claims`, `claim_evidence`, `claim_relations` |
| Profiles / periods | migrations + store | `entity_profiles`, `periods`, `entity_period_links` |
| Extractors | `crates/talaria-judge/` + new claim extractor | Beyond COSMOS: anecdote/fact/debate candidates |
| Dossier | `crates/talaria-api/src/narrative_dossier.rs` | Offline section weave only (flag to disable live API) |
| API filters | `crates/talaria-api/src/routes/` | Filter timeline/geojson by profile, period, epistemic |
| FE filters | `web/src/components/filters/` | Profile × period × epistemic facets |
| Adapters (later) | `crates/talaria-sources/` (new) | Europeana / BnF / Archive.org / ISBN stubs |

---

## Phase 0 — Storage hygiene & dump-only mode

### Task 0.1: Document and verify dual-root layout

**Files:**
- Modify: `README.md` (storage section only if user later asks; prefer `.env.example` comments)
- Modify: `.env.example`

- [x] **Step 1:** Ensure `.env.example` documents:

```bash
# Cold dumps (HDD / S3 mount). Not required at API runtime after ingest.
TALARIA_DATA_ROOT=/mnt/wiki-dump

# Hot DB (must be reachable by API in prod — prefer SSD / managed Postgres)
DATABASE_URL=postgres://postgres:postgres@localhost:5433/talaria_engine_development

# When true, narrative dossier must not call MediaWiki HTTP
TALARIA_OFFLINE_ONLY=false
```

- [x] **Step 2:** Run `cargo run -p talaria-api -- data-init` and confirm dirs under `TALARIA_DATA_ROOT`: `dumps/`, `pages/`, `parquet/`, `wikidata/`.

- [ ] **Step 3:** Commit env example + config parse for `TALARIA_OFFLINE_ONLY` (deferred until user asks).

### Task 0.2: Gate live Wikipedia in dossier

**Files:**
- Modify: `crates/talaria-core/src/config.rs`
- Modify: `crates/talaria-api/src/narrative_dossier.rs`
- Modify: `crates/talaria-api/src/routes/events.rs`

- [x] **Step 1:** Add `offline_only: bool` to `AppConfig` from `TALARIA_OFFLINE_ONLY`.
- [x] **Step 2:** When `offline_only`, skip `fetch_section_claims`; build dossier only from DB evidence + narrative window + stored section text (if any).
- [x] **Step 3:** Manual check: `TALARIA_OFFLINE_ONLY=true` → status reports `offline_only`; dossier skips live MediaWiki.

---

## Phase 1 — Universal entity profiles (Wikidata-shaped)

### Task 1.1: Schema for profiles & periods

**Files:**
- Create: `migrations/005_profiles_periods.sql`
- Modify: `crates/talaria-store/src/lib.rs` (exports)

```sql
-- migrations/005_profiles_periods.sql
CREATE TABLE IF NOT EXISTS periods (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug TEXT NOT NULL UNIQUE,
    label TEXT NOT NULL,
    start_year INT,
    end_year INT,
    kind TEXT NOT NULL DEFAULT 'century'
        CHECK (kind IN ('year', 'decade', 'century', 'era', 'reign', 'custom')),
    wikidata_qid TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS entity_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    profile_qid TEXT,              -- Wikidata occupation/position QID when known
    profile_slug TEXT NOT NULL,    -- e.g. military-leader, singer, head-of-state
    profile_label TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'occupation'
        CHECK (kind IN ('occupation', 'position', 'field', 'custom')),
    confidence DOUBLE PRECISION NOT NULL DEFAULT 0.8,
    source_system TEXT NOT NULL DEFAULT 'wikidata',
    UNIQUE (entity_id, profile_slug, kind)
);

CREATE TABLE IF NOT EXISTS entity_periods (
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    period_id UUID NOT NULL REFERENCES periods(id) ON DELETE CASCADE,
    PRIMARY KEY (entity_id, period_id)
);

CREATE INDEX IF NOT EXISTS idx_entity_profiles_slug ON entity_profiles (profile_slug);
CREATE INDEX IF NOT EXISTS idx_entity_profiles_entity ON entity_profiles (entity_id);
```

- [x] **Step 1:** Add migration; `cargo run -p talaria-api -- migrate`.
- [x] **Step 2:** Seed a small `periods` set (centuries 15–21 + eras) via `seed_default_periods` + `scripts/seed-demo-profiles.sh`.
- [ ] **Step 3:** Commit (deferred until user asks).

### Task 1.2: Store API for profiles

**Files:**
- Create: `crates/talaria-store/src/profiles.rs`
- Modify: `crates/talaria-store/src/lib.rs`

- [x] **Step 1:** Implement `upsert_entity_profile`, `list_entity_profiles`, `list_periods`, `link_entity_period`.
- [x] **Step 2:** Verified via seed + API: Madonna singer profiles + Napoleon military/head-of-state.
- [ ] **Step 3:** Commit (deferred until user asks).

### Task 1.3: API + FE filters (profiles / periods)

**Files:**
- Modify: `crates/talaria-api/src/routes/events.rs` (timeline/geojson query params)
- Modify: `crates/talaria-api/src/routes/entities.rs`
- Modify: `web/src/components/filters/explorer-event-filters.tsx`
- Modify: `web/src/lib/api.ts`

- [x] **Step 1:** Add query params `profile_slug`, `period_slug`, keep existing epistemic/type filters.
- [x] **Step 2:** Timeline SQL joins `entity_profiles` / `entity_periods` when filters set.
- [x] **Step 3:** FE: profile + period facets from API.
- [x] **Step 4:** Verify with two seeded entities of different profiles.
- [ ] **Step 5:** Commit (deferred until user asks).

---

## Phase 2 — Wikidata dump ingest (offline occupations)

### Task 2.1: Stream Wikidata JSON dump → occupations/positions

**Files:**
- Create: `crates/talaria-wikidata/src/dump.rs`
- Modify: `crates/talaria-wikidata/src/lib.rs`
- Modify: `crates/talaria-api/src/cli.rs` (new command `wikidata-ingest`)
- Modify: `crates/talaria-dump/src/layout.rs` (add `wikidata/` under data root)

- [x] **Step 1:** Place dump at `$TALARIA_DATA_ROOT/dumps/wikidata-*-all.json.bz2` (or a sliced test file of ~1k humans).
- [x] **Step 2:** Implement streaming parse: for `P31=Q5`, read `P106`, `P39`, `P569`, `P570`, sitelinks; upsert `entities.qid` + `entity_profiles`.
- [x] **Step 3:** CLI: `cargo run -p talaria-api -- wikidata-ingest --limit 1000`.
- [x] **Step 4:** Confirm Madonna-like test entity gets multiple profiles without live HTTP.
- [ ] **Step 5:** Commit (deferred until user asks).

### Task 2.2: Link Wikipedia pages ↔ QID via sitelinks

**Files:**
- Modify: `crates/talaria-store/src/entities.rs`
- Modify: ingest path after WP extract

- [x] **Step 1:** After WD ingest, match entity wikipedia titles to sitelinks; set `entities.qid`.
- [x] **Step 2:** Entity search prefers local QID/profiles before any live Wikidata fallback (`offline_only` + local-first search).
- [ ] **Step 3:** Commit (deferred until user asks).

---

## Phase 3 — Claim layer (density beyond COSMOS)

### Task 3.1: Claims schema

**Files:**
- Create: `migrations/006_claims.sql`

```sql
CREATE TABLE IF NOT EXISTS claims (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    claim_kind TEXT NOT NULL
        CHECK (claim_kind IN (
            'fact', 'anecdote', 'context', 'theory', 'controversy',
            'debate_stance', 'attribution', 'life_event'
        )),
    text TEXT NOT NULL,
    epistemic_status TEXT NOT NULL DEFAULT 'attested',
    relation_to_subject TEXT NOT NULL DEFAULT 'direct'
        CHECK (relation_to_subject IN ('direct', 'indirect', 'historiography', 'legacy')),
    event_time TIMESTAMPTZ,
    place_label TEXT,
    confidence DOUBLE PRECISION NOT NULL DEFAULT 0,
    canonical_event_id UUID REFERENCES canonical_events(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS claim_evidence (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    claim_id UUID NOT NULL REFERENCES claims(id) ON DELETE CASCADE,
    source_system TEXT NOT NULL,
    locator TEXT,                 -- oldid URL, ISBN, Europeana ID…
    quote TEXT,
    sentence_id UUID REFERENCES sentences(id) ON DELETE SET NULL,
    confidence DOUBLE PRECISION NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS claim_relations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    from_claim_id UUID NOT NULL REFERENCES claims(id) ON DELETE CASCADE,
    to_claim_id UUID NOT NULL REFERENCES claims(id) ON DELETE CASCADE,
    relation TEXT NOT NULL
        CHECK (relation IN ('supports', 'contradicts', 'debates', 'qualifies')),
    UNIQUE (from_claim_id, to_claim_id, relation)
);
```

- [x] **Step 1:** Migrate.
- [x] **Step 2:** Store CRUD helpers.
- [ ] **Step 3:** Commit (deferred until user asks).

### Task 3.2: Claim extraction pass (post-sentences)

**Files:**
- Create: `crates/talaria-api/src/claim_extract.rs` (or `crates/talaria-judge` extension)
- Modify: judge to optionally attach claims without requiring place+time

- [x] **Step 1:** Rule pass: classify sentences into `claim_kind` via cues (anecdote/theory/controversy) + always keep COSMOS life events as `life_event`.
- [x] **Step 2:** Persist `claim_evidence` with quote + wiki locator.
- [x] **Step 3:** Soft-accept claims missing geo (unlike current event judge).
- [x] **Step 4:** Test on local bios (claims extract + entity claims API).
- [ ] **Step 5:** Commit (deferred until user asks).

### Task 3.3: Persist Wikipedia sections from dump

**Files:**
- Modify: `crates/talaria-text/` (section split)
- Modify: `crates/talaria-store/` + migration `007_sections.sql`
- Modify: `narrative_dossier.rs` to read local sections

- [x] **Step 1:** Store `wiki_sections(wiki_page_id, ordinal, title, text)`.
- [x] **Step 2:** Dossier prefers local section matching event type (birth→Naissance/Early life).
- [x] **Step 3:** With `TALARIA_OFFLINE_ONLY=true`, dossier quality remains usable (seeded FR Naissance).
- [ ] **Step 4:** Commit (deferred until user asks).

---

## Phase 4 — Explorer surfaces for claims & debates

### Task 4.1: API — claims & conflicts for an entity/event

**Files:**
- Create: `crates/talaria-api/src/routes/claims.rs`
- Modify: `routes.rs`

- [ ] **Step 1:** `GET /api/v1/entities/{id}/claims?kind=&epistemic=`
- [ ] **Step 2:** `GET /api/v1/events/{id}` includes related claims + relations.
- [ ] **Step 3:** Commit.

### Task 4.2: FE — filters + claim panel

**Files:**
- Modify: `web/src/pages/explorer-page.tsx`
- Modify: `web/src/components/detail/event-detail-card.tsx`
- Create: `web/src/components/claims/claims-list.tsx`

- [ ] **Step 1:** Show claims grouped by kind under event detail.
- [ ] **Step 2:** Filter bar: profile, period, claim kind, epistemic.
- [ ] **Step 3:** Manual UX pass on at least two different profile entities.
- [ ] **Step 4:** Commit.

---

## Phase 5 — Multi-source adapters (after core density)

### Task 5.1: Source adapter trait

**Files:**
- Create: `crates/talaria-sources/` workspace crate

```rust
// crates/talaria-sources/src/lib.rs
pub struct SourceDocument {
    pub source_system: String, // "europeana" | "bnf" | "archive_org" | "isbn"
    pub external_id: String,
    pub title: Option<String>,
    pub locator: String,
    pub license: Option<String>,
    pub text: Option<String>,
}

#[async_trait::async_trait]
pub trait SourceAdapter: Send + Sync {
    fn system_id(&self) -> &'static str;
    async fn search_person(&self, name: &str, qid: Option<&str>) -> anyhow::Result<Vec<SourceDocument>>;
}
```

- [ ] **Step 1:** Stub adapters returning empty/`todo` with config keys.
- [ ] **Step 2:** Europeana Entity/Search adapter first (Agent + TimeSpan alignment).
- [ ] **Step 3:** Map hits → `claim_evidence` with `source_system` + locator.
- [ ] **Step 4:** Commit.

---

## Phase 6 — Production deploy posture

### Task 6.1: Deploy checklist (ops, not feature code)

- [ ] Postgres on managed service or VPS SSD with automated backups.
- [ ] API + web on same VPS or containers; `DATABASE_URL` points to managed DB.
- [ ] Dumps: either upload to S3 and run ingest on a worker, or ingest locally then `pg_dump`/`pg_restore` to prod (no HDD required in prod).
- [ ] Set `TALARIA_OFFLINE_ONLY=true` in production API.
- [ ] Health checks do not depend on `TALARIA_DATA_ROOT` existing.

---

## Suggested execution order (working software each step)

1. Phase 0 (offline gate + storage clarity)  
2. Phase 1 (profiles/periods schema + filters) — FE value early  
3. Phase 2 (Wikidata dump → real multi-profiles)  
4. Phase 3 (claims + local sections) — density  
5. Phase 4 (UI for claims)  
6. Phase 5–6 when core density matches POC intent  

---

## Self-review

| Requirement from discussion | Covered by |
|-----------------------------|------------|
| Dump-first, avoid API cascades | Phase 0, 2, 3.3 |
| Dense facts/anecdotes/theories/debates + evidence | Phase 3–4 |
| Universal personalities / profiles | Phase 1–2 |
| Date / century / period filters | Phase 1 |
| Wikidata/Wikipedia/Europeana inspiration | Phase 2, 5 |
| Local HDD dumps now, deploy later | Storage decision + Phase 6 |
| Production DB on home HDD? | Explicitly rejected; migrate hot DB to hosted SSD |

No TBD placeholders for Phase 0–3 schemas; Phase 5 adapters intentionally stubbed as interfaces first.

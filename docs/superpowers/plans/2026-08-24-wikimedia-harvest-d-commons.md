# Wikimedia harvest D — Commons MediaInfo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Commons stub with a MediaInfo connector that stores attributed `media_assets` (thumb URL + license) and **never** writes Events or historical Claims.

**Architecture:** Discover files from Wikidata P18 (and P1442/P109), then ≤10 `prop=images` on already-ingested Wikipedia pages, then `commonswiki` sitelink listed files only. Fetch MediaInfo + `imageinfo`. Reject rows without `attribution_text`. No original bytes.

**Tech Stack:** Rust, sqlx, `talaria-sources`, `talaria-store`, `talaria-api`.

## Global Constraints

- No Commons full-text search.
- No recursive category crawl.
- No original file download.
- No Commons SPARQL as provenance.
- No hero-image UI rewiring.
- Tests: fixtures only.
- `source-status` commons → `extraction_ready`.

---

### Task 1: License/attribution parse + MediaInfo mapping

**Files:**
- Create: `crates/talaria-sources/src/connectors/commons.rs`
- Modify: `crates/talaria-sources/src/connectors/mod.rs`
- Test: inline in `commons.rs`

**Interfaces:**
```rust
pub struct CommonsAsset {
    pub commons_file: String,
    pub mid: Option<String>,
    pub sha1: Option<String>,
    pub mime: Option<String>,
    pub license: Option<String>,
    pub attribution_text: String,
    pub thumb_url: Option<String>,
    pub depicts_qids: Vec<String>,
    pub revision_id: Option<String>,
    pub rights_normalized: String, // open | restricted | metadata_only | unknown
}
pub fn parse_mediainfo(entity: &Value, imageinfo: Option<&Value>) -> Option<CommonsAsset>;
```

`parse_mediainfo` returns `None` if `attribution_text` would be empty (spec: reject thumb without attribution).

Attribution: MediaInfo P2091/P170 labels, or `Artist`/`Author` + `License` from `extmetadata` (`imageinfo.extmetadata.Artist.value` + `LicenseShortName`). Strip HTML tags with a small helper.

Depicts: claims P180 item ids.

Thumb: `imageinfo[0].thumburl` (request `iiurlwidth=640` live).

- [ ] **Step 1: Failing test**

```rust
#[test]
fn mediainfo_requires_attribution() {
    let entity = serde_json::json!({"id":"M1","statements":{}});
    let info = serde_json::json!([{"thumburl":"https://upload.wikimedia.org/x.jpg","mime":"image/jpeg"}]);
    assert!(parse_mediainfo(&entity, Some(&info)).is_none());
}

#[test]
fn mediainfo_ok() {
    let entity = serde_json::json!({"id":"M123","statements":{
        "P180":[{"mainsnak":{"datavalue":{"value":{"id":"Q517"}}}}]
    }});
    let info = serde_json::json!([{
        "thumburl":"https://upload.wikimedia.org/thumb/x.jpg",
        "mime":"image/jpeg",
        "sha1":"abc",
        "extmetadata":{
            "Artist":{"value":"Louvre"},
            "LicenseShortName":{"value":"CC BY-SA 4.0"}
        }
    }]);
    let a = parse_mediainfo(&entity, Some(&info)).unwrap();
    assert!(a.attribution_text.contains("Louvre"));
    assert_eq!(a.depicts_qids, ["Q517"]);
    assert_eq!(a.rights_normalized, "open");
}
```

- [ ] **Step 2: FAIL then implement**
- [ ] **Step 3: `cargo test -p talaria-sources mediainfo_ok`**
- [ ] **Step 4: Commit** `feat(sources): Commons MediaInfo parser with required attribution`

---

### Task 2: `media_assets` table

**Files:**
- Create: `migrations/025_media_assets.sql`
- Create: `crates/talaria-store/src/media.rs`
- Modify: `crates/talaria-store/src/lib.rs`

```sql
CREATE TABLE IF NOT EXISTS media_assets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    commons_file TEXT NOT NULL,
    mid TEXT,
    sha1 TEXT,
    mime TEXT,
    license TEXT,
    attribution_text TEXT NOT NULL,
    thumb_url TEXT,
    depicts_qids TEXT[] NOT NULL DEFAULT '{}',
    revision_id TEXT,
    rights_normalized TEXT NOT NULL DEFAULT 'unknown',
    entity_id UUID REFERENCES entities(id) ON DELETE SET NULL,
    corpus_document_id UUID REFERENCES corpus_documents(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (commons_file, sha1)
);
```

If `sha1` is null, uniqueness on `commons_file` where sha1 is null (`UNIQUE NULLS NOT DISTINCT` on PG 15+ or unique index on `commons_file` where sha1 is null). Prefer `UNIQUE (mid)` when mid present.

```rust
pub async fn upsert_media_asset(pool: &PgPool, row: &MediaAssetInsert) -> anyhow::Result<Uuid>;
```

- [ ] Implement + `cargo check -p talaria-store`
- [ ] Commit `feat(store): media_assets table`

---

### Task 3: Connector discover/fetch + ingest wire-up

**Files:**
- Modify: `crates/talaria-sources/src/connectors/commons.rs` (`SourceConnector`)
- Modify: `crates/talaria-sources/src/connectors/mod.rs` (register live, remove from stub loop)
- Modify: `crates/talaria-api/src/ingest.rs` (persist media, **no** Event extractors for Commons)
- Modify: `crates/talaria-api/src/lot_e.rs` (`connector_status_json` commons → `extraction_ready`; after Wikidata fetch, enqueue P18 files)
- Modify: `AGENTS.md` one line: Wikisource/Commons extraction_ready (not stub)

**Discover:**
1. From `ResolvedSubject` / Wikidata metadata: file titles in P18.
2. Do not search Commons.
3. `document_type`: `media_caption`.
4. `max_documents` already min(10) in plan.rs.

**Fetch:** Commons API `action=wbgetentities` on `M-id` if known; else `action=query&titles=File:...&prop=imageinfo|revisions&iiprop=url|size|mime|sha1|extmetadata&iiurlwidth=640`. Parse; if `parse_mediainfo` is None, skip (`commons_unlicensed` counter in ingest metrics if easy; else `tracing::info`).

**Ingest:** `source_kind=commons` → upsert `media_assets` + optional `document_snapshots` with MediaInfo JSON (`source_type=commons`). **Do not** call Event extractors.

No P18 and no page images → 0 assets (do not invent).

- [ ] **Step 1: Test parse-only already done; add `document_from_p18("File:x.jpg")`**
- [ ] **Step 2: Implement connector + registry**
- [ ] **Step 3: Wire ingest + lot_e P18 enqueue** (parse claims P18/P1442/P109 mainsnak string or entity id titled File)
- [ ] **Step 4: `cargo test -p talaria-sources && cargo test -p talaria-api --offline`**
- [ ] **Step 5: Commit** `feat(ingest): Commons MediaInfo assets without binaries or events`

---

## Spec coverage (D)

| Spec | Task |
|---|---|
| MediaInfo + imageinfo, no binaries | 1, 3 |
| attribution required | 1 |
| P18 / wiki images / sitelink listed files | 3 |
| media_assets | 2 |
| never Event/Claim | 3 |
| skip unlicensed / broken P18 | 1, 3 |
| source-status | 3 |
| no SPARQL / no UI | not implemented |

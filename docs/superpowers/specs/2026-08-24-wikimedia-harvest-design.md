# Wikimedia harvest (Wikipedia → Wikidata → Wikisource → Commons)

Date: 2026-08-24  
Status: approved — 2026-08-24  
Decisions: person-first harvest on the existing quality spine; live API default; local dump optional; no third event pipeline.

## Goal

Ingest **more usable Wikimedia evidence** for a person without a Wikimedia data lake. Each project has one role. Wikis produce snapshots, fragments, mentions, citations, claims, and media records. They do **not** write `canonical_events` except through the existing quality gates.

Success for a subject: denser, sourceable life-trace evidence (sections, links, refs, full Wikidata claims, primary Wikisource text, attributed Commons files) with **zero** extra events invented from undated properties, captions, or primary-source prose.

Napoleon remains a regression fixture, not a density floor.

## Non-goals

- Full `latest-all.json.bz2` or Wikipedia SQL dumps as the default path
- Complete revision histories, talk/user pages, Wikivoyage, Wikinews, Wikiquote (later)
- Commons/Wikisource binaries (DjVu, original files)
- Parsoid / HTML as authority (wikitext + revision_id)
- WDQS or Commons SPARQL as provenance
- Changing quality gates, `occurrence_key`, append-only events, or `map_eligible`
- Requalifying `pipeline='legacy'` into quality
- A third event pipeline
- Event-detail hero UI (already specified separately; D may later feed it, not in this spec)

## Approach chosen

Person-first on the quality spine (extend connectors + snapshots). Live Wikimedia APIs are the default. A local dump is optional enrichment when the file exists. Rejected: bulk dump lake; connectors that discover without fragmenting.

## Architecture

Three layers, no new event assembler:

```text
Wikimedia (Action API, or local dump if present)
  → immutable snapshot (document_snapshots + hash + revision_id + QID)
  → sourcable fragments / statements / media rows
  → Entity / Place / Claim
  → EventCandidate only if date + type + participation
  → existing quality gates → canonical_events
```

| Source | Role | Becomes Event? |
|---|---|---|
| Wikidata | identity, claims, places, sitelinks | only promotion rules in B |
| Wikipedia FR+EN | narration, infobox, page graph | candidates from fragments, then gates |
| Wikisource FR | primary proof | never auto |
| Commons | MediaInfo + attribution | never (media, not fact) |

Storage is **additive**. Extend `document_snapshots.metadata` and `document_fragments`. Add `wikibase_statements` and `media_assets`. Never silently rewrite an existing event.

Implementation order is **A → B → C → D**. Each phase ships a testable ingest. Do not start B until A’s tests pass, and so on.

## Shared constraints

- Person-first: harvest neighborhood of one QID, not whole wikis.
- Languages for A: `fr` and `en` only.
- Wikisource: `fr.wikisource.org` only.
- User-Agent must identify Talaria; polite delays already used in lot E stay.
- Tests are fixtures / recorded JSON; no live network in CI.
- `source-status` must not claim `extraction_ready` for a stub.
- STATEMENT TSV may remain as a **derived** test fixture format; it is not the Wikidata canonical store after B.

---

## A — Wikipedia wikitext → fragments

### Problem

Live ingest uses `explaintext`. Wikitext is sometimes fetched and then ignored as the extraction corpus. Lot E often stores **one fragment = whole article**. Infobox/section parsers exist in `talaria-text` but the live path does not drive them.

### Behavior

The Wikipedia snapshot authority is **wikitext + `revision_id`**. Extractors read **fragments**, not the whole plaintext page.

Connector fetch: `prop=revisions|info|pageprops|coordinates`, `rvprop=content|ids`, plus `pageprops.wikibase_item`. Plaintext extract, if kept, goes in `metadata.plain_extract` (display only), never as `document_snapshots.text`.

`document_snapshots.text` = wikitext. Metadata includes `page_id`, `qid`, `lang`, `redirects`, `page_coords`, raw infobox when parsed.

`document_fragments`:

- Widen `fragment_kind` CHECK to `section | infobox | sentence | clause` (keep existing `sentence`/`clause`).
- Relax the current CHECK that allows only sentence-without-clause_index or clause-with-clause_index: `section` and `infobox` have `clause_index` NULL and optional `parent_fragment_id`.
- Add `metadata JSONB` (default `{}`): `section_path`, `internal_links[{surface,target_title,qid}]`, `citations[{ref_name,text}]`.
- Offsets are character offsets into `snapshot.text`.
- Replace “one fragment = entire article” on the live quality path.

Parser (`talaria-text`): reuse section split, sentence split, infobox fields, wikilinks. Add per-fragment links and `<ref>` attachment. Resolve link titles → QID in batches via `wbgetentities` **before** NER. Unresolved links keep `target_title` and `qid: null`.

FR and EN ingest both; merge by QID in B. Do **not** drop “duplicate” fragments across languages before extraction.

Dump XML remains optional: same parser when a local pages-articles dump exists. No SQL `pagelinks`/`categorylinks` in A.

### Errors

| Case | Behavior |
|---|---|
| No wikitext | snapshot `source_form=plain`, one page-level fragment, metric `wikitext_missing` |
| Partial parse | keep snapshot; store only good fragments |
| Redirect | resolved title; chain in metadata |

A does not assemble Events. Existing extractors + gates still do.

### Tests

- Napoleon FR fixture wikitext: multiple sections; sentence containing crowning 1804; link `Paris`; at least one `<ref>`.
- Infobox fragment + existing birth/death candidates.
- `ingest-quality` fixture: more than one sentence fragment per article.

---

## B — Wikidata full claims

### Problem

The connector flattens ~9 PIDs into `STATEMENT` TSV. Qualifiers, ranks, references, snaktype, and most properties are dropped. `parse_wikidata_year` takes four characters (breaks BCE). The JSON dump path emits humans (Q5) + occupations, not full claims. The structured extractor can promote every dated line to an event candidate.

### Behavior

Canonical store = **full entity JSON** on the snapshot (`raw_metadata` / snapshot text JSON) plus normalized `wikibase_statements`:

- `qid`, `guid`, `property`, `rank`, `snaktype`
- `value_json` (item, time, globecoordinate, quantity, string, …)
- `qualifiers_json`, `references_json`
- `revision_id`

Wikibase time is typed: precision, calendar model, **negative years**. Deprecated ranks are stored and **excluded** from active projections. Preferred beats normal for identity (P569, P570, P19, P20).

Live default: `wbgetentities` for the subject QID. Dump, if present, is **filtered to the subject neighborhood** (subject + item-valued claims + sitelinks), not `latest-all`.

`STATEMENT` lines remain for existing fixture tests. After B, `structured` runs only on **promoted** claims.

### Event promotion (closed list)

Promote to `EventCandidate` only if:

| Promote | Stay Claim |
|---|---|
| P569 / P570 (P570 only if present) | P551 / P937 **without** P580/P582/P585 |
| P793 + a date | P106, P27, P18, external ids |
| P39 / P26 / P69 **with** a date | P19/P20 alone (place → Place; date from P569/P570) |
| P607 / P1344 **with** a date **and** subject as participant | any other dated property |

`somevalue` / `novalue`: store statement, no Event. Missing neighbor QID: store QID, resolve label later.

### Tests

- Q517 JSON: P551 with P580/P582 → Claim + `residence` EventCandidate; P106 → Claim only.
- BCE birth: negative year, not coerced to 1 January.
- Tiny dump: one human emitted with full claims, not occupation-only.

---

## C — Wikisource FR

### Problem

`SourceKind::Wikisource` is a stub (`NotConfigured`). The plan already budgets Wikisource, but ingest never fetches.

### Behavior

Real connector for **fr.wikisource.org** only. Output is `corpus_documents` + snapshots + fragments. Academic status `primary_source`. **Zero auto Events.**

Genre (`letter`, `speech`, `treaty`, `law`, `memoir`, `periodical`, `narrative`) is metadata, not an event type.

Discovery (respect existing per-source document budget, cap 15):

1. Wikidata sitelink `frwikisource` (work or author).
2. `Auteur:` page + linked main-namespace pages.
3. Action API search if no sitelink.
4. `siteinfo` for `Livre` / `Page` namespaces (no hardcoded NS numbers).

Default fetch: **main transclusion** (readable text). Namespace `Page:` only with an explicit flag (out of default person ingest).

Keep: title, author, edition, date, language, work/author QID, pagination, ProofreadPage level, OCR vs corrected, scan **URL** (not file bytes).

Fragments may be chapter / logical page. `problematic` proofread → fragment `needs_review`, no claim. Empty transclusion → try Index; else metadata-only snapshot. No sitelink and no search hit → skip, invent nothing.

Quality extractors that emit EventCandidates **must not** run on `source_kind=wikisource` in C.

### Tests

- Fixture Index + main transclusion → one `corpus_document`, paged fragments, `primary_source`.
- Mocked live discover is non-empty; `source-status` wikisource = `extraction_ready`.
- Subject ingest: Wikisource adds **0** `canonical_events`.

---

## D — Commons MediaInfo (no binaries)

### Problem

Commons is a stub. Event-detail thumbs fetch Wikipedia REST live and do not persist attribution. ChatGPT correctly treats Commons as structured media, not an image CDN dump.

### Behavior

Real Commons connector. Persist **metadata + license**, never original bytes. A media row is never an Event or a historical Claim.

Discovery (person-first, no Commons full-text search):

1. Wikidata P18 (and P1442/P109 if present) → `File:` + M-id.
2. Images on already-ingested Wikipedia pages (`prop=images` / imagelinks), budget ≤ 10 files.
3. `commonswiki` sitelink (category or gallery): metadata for **listed files only**, no recursive category crawl.

Persist in additive `media_assets`: `commons_file`, `mid`, `sha1`, `mime`, `license`, `attribution_text`, `thumb_url` (CDN thumb via `iiurlwidth`, not original), `depicts_qids`, `revision_id`, `rights_normalized`. Optional link to `entity_id` / `corpus_document_id` (`depicts` / `illustrates`). MediaInfo JSON may live on `document_snapshots` with `source_type=commons`.

A thumb URL **without** `attribution_text` is rejected. Unreadable license → skip + metric `commons_unlicensed`. Missing MediaInfo → wikitext license only. Broken P18 → skip. P180 aligns an entity; it does not invent a dated place. No P18 and no page images → **0** invented assets.

Out of D: original file download, Commons SPARQL as provenance, hero-image UI rewiring.

### Tests

- Fixture MediaInfo + `imageinfo` → one `media_assets` row with attribution, **0** Events.
- `source-status` commons = `extraction_ready`.
- Ingest without P18 → 0 assets.

---

## Error handling (all phases)

- Network/429: existing retry/backoff; skip document, continue subject.
- Connector not configured: `NotConfigured` only for remaining stubs (VIAF/ISNI/IdRef), not for A–D after ship.
- Idempotency: snapshot unique on `(source_type, source_uri, content_hash)` unchanged; statements unique on `guid` + `revision_id`; media unique on `(commons_file, sha1)` or `mid`.
- Failures increment metrics; they do not abort the whole person ingest.

## Testing strategy

- Unit tests in `talaria-text` (A parser), `talaria-wikidata` (B time + promotion), new connector modules (C/D).
- Fixture ingest tests under `talaria-sources` / `talaria-api` with recorded JSON, no network.
- After each phase: `cargo test -p` the touched crates; `source-status` JSON reflects readiness.

## Rollout

Ship A, then B, then C, then D as sequential PRs or sequential local commits. Do not merge a phase that still reports `stub` in `source-status` while claiming it is done.

## Risks

- Wikitext offsets vs plaintext extractors: extractors must take fragment `.text` (plain or wikitext per extractor contract). Infobox extractor already prefers `wikitext`.
- Promotion list too strict: missed offices without dates stay Claims (correct). Too loose: revert to the closed list, do not add PIDs ad hoc.
- Wikisource transclusion cost: cap documents; default skip `Page:`.
- Commons license parsing: skip rather than guess.

# Debate source stack (OpenAlex first)

Date: 2026-08-13
Status: approved (chat) — scholarly APIs, no web crawl, no LLM as historian

## Goal

Find **historian debates** (origin theories, controversies, historiography) from serious bibliographic indexes. Persist title + abstract as `corpus_documents`. Never write `canonical_events`. `historiography-extract` already turns those titles/abstracts into `soft_claims`.

## Non-goals (this slice)

- HAL, Persée, Gallica OCR, Crossref, Open Library (stubs remain)
- HTML scraping / allowlisted crawler
- LLM scout or claim generation
- Explorer Débats tab
- Full-text PDF ingest

## Source ladder

1. Wikipedia historiography **pages** (seeds) — already extractable as wiki sections
2. **OpenAlex** — works API, title + reconstructed abstract
3. theses.fr — already implemented
4. **Internet Archive** — `advancedsearch` + `metadata` (texts). No key.
5. **Europeana** — Search API v2. Live needs `EUROPEANA_API_KEY`.
6. **BnF catalogue** — SRU Dublin Core. Notices only, not Gallica OCR.
7. Later: HAL-SHS, Persée OAI, Gallica IIIF/OCR, Open Library

No Ancestry, forums, YouTube, JSTOR scrape.

## OpenAlex

- Discover: `GET /works?search=` with subject label **and** debate terms (`origins`, `historiography`, `controversy`, `origines`, `nationality`, `birthplace`, …)
- Fetch: `GET /works/{W…}` (fixture: `fixtures/open_alex/details/{id}.json`)
- Store: metadata + abstract only. No PDF bytes.
- Identity: OpenAlex work id (`W…`) + DOI when present.
- Epistemic: bibliographic resource. Peer-reviewed article ≠ established fact.
- Polite pool: `User-Agent` + optional `mailto` (`OPENALEX_MAILTO`).
- Tests: fixtures only, no network.

## Pipeline

```
corpus-ingest --providers open_alex → corpus_documents
historiography-extract → soft_claims (theory / controversy)
intuition-plan → questions (no map pins)
```

A Catalan/Jewish/Swiss origin thesis is a **proposition**, never a new birth point.

## CLI

```
talaria corpus-ingest --subject "Christophe Colomb" --providers open_alex --fixture true
talaria historiography-extract --subject "Christophe Colomb"
```

`--live` hits `https://api.openalex.org` (requires network). Default providers stay `theses_fr`.

# Source Expansion: Explorer Evidence vs Grounding vs Agora

This document explains how different source connectors are classified and wired into the Talaria pipeline.

## Source Classification

### 1. Explorer Evidence Sources (Vague A)

These sources **reinforce existing canonical events** with additional evidence. They NEVER create new map pins — they only add evidence to already-gated events via occurrence_key matching.

| Source | Status | Description |
|--------|--------|-------------|
| HAL | Live | French open archive scholarly works |
| Gallica | Live | BnF digitized books, press, correspondence |
| BnF | Live | Bibliothèque nationale de France catalogue |
| Persée | Live | French academic journals (OAI-PMH) |
| theses.fr | Live | French theses metadata |
| OpenAlex | Live | Scholarly works API |

**Evidence Wiring:**
- Corpus connectors are queried during `run_person_ingest`
- Documents are matched to existing canonical events by year, place, and event type
- Evidence is inserted idempotently via `insert_person_quote_evidence`
- Same occurrence + same source = no-op (idempotent)

### 2. Grounding Sources (Vague B)

These sources provide **place-identity resolution** and **structured life facts**. They help resolve place names to authoritative identifiers but do NOT provide coordinates for map pins.

#### Place-Identity Grounding (PlaceIdentityResolver trait)

These are **not** SourceConnectors. They implement the `PlaceIdentityResolver` trait in `talaria-sources/src/place_identity.rs` and are used in the grounding chain (mention → identity → geocode).

| Source | Status | Implementation |
|--------|--------|----------------|
| Alias Gazetteer | Live | Built-in offline gazetteer with known Napoleon-era places |
| Getty TGN | Live | SPARQL queries to `vocab.getty.edu/sparql` |
| WHG | Gated (requires `WHG_API_TOKEN`) | REST API to `whgazetteer.org/api` |

**Usage:**
- These resolvers are chained via `CompositeIdentityResolver`
- Order: Alias Gazetteer → TGN → WHG
- They return `PlaceIdentity` with Wikidata QID alignment when available
- Coordinates come LATER via P625 lookup on the resolved QID

#### Heritage Place Sources (SourceConnector)

| Source | Status | Description |
|--------|--------|-------------|
| POP/Mérimée | Live | French heritage monuments with WGS84 coords |

**Usage:**
- POP/Mérimée provides WGS84 coordinates for heritage monuments
- These are monument-centric lookups, not general place grounding

#### Structured Life Facts

| Source | Status | Description |
|--------|--------|-------------|
| FranceArchives | Live | French archival EAD/EAC-CPF records |
| Deutsche Biographie + GND | Live | German biographical data via lobid.org |

**Usage:**
- These sources provide structured birth/death/office facts
- Facts flow through existing gates (apply_gates, SubjectAttribution)
- They create event candidates, not directly canonical events

### 3. Agora-Only Sources (Deferred)

These sources feed the **claims/soft_claims tables**, NOT the map/timeline. They are for historiographic debate, not factual events.

| Source | Status | Description |
|--------|--------|-------------|
| ISIDORE | Deferred | French research discovery |
| OpenEdition | Deferred | French academic publisher |
| Chronicling | Deferred | Historical newspaper OCR |

## Pipeline Rules (AGENTS.md Compliance)

1. **Explorer map/timeline = pipeline='person' only**
2. **Never invent coordinates / map pins**
3. **Pin only if TypedTime + real lat/lon + evidence**
4. **Extra sources reinforce same occurrence_key via idempotent evidence — NEVER auto-create a new pin from a catalogue hit alone**
5. **Grounding layers (TGN/WHG) resolve place identity; coords still from P625 / gazetteer / page coords**
6. **Agora sources → claims/soft_claims, not pins**

## Progressive UX Integration

The corpus evidence enrichment runs **after** the initial Wikipedia/Wikidata ingest, meaning:

1. First paint: Wikipedia + Wikidata events appear immediately
2. Enrichment: Corpus sources add evidence to existing events asynchronously
3. No blocking: The progressive UX agent can render timeline/map with initial data while enrichment runs

The enrichment stats are returned in the `run_person_ingest` response under `corpus_enrichment`:

```json
{
  "corpus_enrichment": {
    "sources_queried": 6,
    "documents_discovered": 42,
    "documents_matched": 15,
    "evidence_added": 18,
    "errors": 0
  }
}
```

## Connector Status Reference

Run `talaria source-status` to see the current maturity of all connectors:

- **Live**: Fully implemented with fetch/parse/extract
- **Stub**: Interface only, requires credentials or further implementation
- **Extraction-ready**: Can fetch/parse but not extract events (e.g., Wikisource for proof/media)

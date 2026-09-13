# Progressive search / map UX

Product-truth notes for the progressive explorer ingest UX (PR #19).

## Contracts (do not break)

- **Explorer** (map / timeline) ≠ **Agora** (catalogs / opinions).
- Extra sources = **evidence** on an existing `occurrence_key` — **never** auto-create a new map pin.
- `map_eligible` only when geom is resolved; otherwise timeline-only.
- TypedTime keeps **kind × precision**; SQL columns (`date_precision`, `coord_precision_level`) are **projections**, not a second model.
- Explorer ingest stays `pipeline='person'` only.

## Progressive layers (product intent)

| Layer | User sees | Must not imply |
|-------|-----------|----------------|
| Suggestions | Instant entity search (local DB + Wikidata) | Full corpus / catalog density |
| t0 | Profile + birth/death pins when ready | Complete biography |
| t1 | Wikipedia / WDQS timeline + more pins | Institutional catalog coverage |
| Later | "Enriching…" while background continues | Lowered quality gates |

Partial states to label clearly in UI copy:

- **Timeline without map** — events with typed time but unresolved place / no geom.
- **Imprecise pin** — geom present but coarse `coord_precision_level` / large `uncertainty_radius_m`.
- **One best search match ≠ rich corpus** — show provenance (local entity vs Wikidata suggestion).

## Precision badge fields (P0)

Status polling may expose `precise_dates`, `precise_coords`, `evidence_count`. Treat these as **diagnostics**, not density targets. Do not invent pins to raise the counters.

## Catalogs / new bases

Institutional connectors (Gallica, BnF, Persée, HAL, …) remain **out of explorer map pins** until an explicit P1 brief. When they later reinforce facts, they add evidence only.

For each new source PR, document: protocol, lane (explorer evidence / Agora / both), geo fields, time fields, maturity, and `source-status` string.

## Out of scope unless explicitly briefed

- Lowering lifespan / attribution gates to get "more points".
- Inventing coordinates (Nominatim, etc.) without a traced coordinate source.
- Promising catalog density on the map before explorer wiring exists.

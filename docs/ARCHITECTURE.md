# Architecture

Talaria uses a modular monolith on Rails 8 with explicit service namespaces.

## Principles
1. Two-layer model: raw/staging and canonical.
2. Raw layer is append-only where possible; snapshots version source artifacts.
3. LLM output is advisory only; deterministic promotion rules gate canonical writes.
4. Canonical event/claim records remain evidence-traceable.
5. Canonical projections are rebuildable from raw + promotion rules.
6. PostGIS geospatial data is first-class with GiST indexes.

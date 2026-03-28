# Pipelines

1. Ingestion creates run + raw document + source snapshot.
2. Fragmentation tokenizes document into ordinal fragments.
3. Extraction writes raw candidates.
4. AI judgment service records advisory judgments.
5. Resolution attaches deterministic place resolution candidates.
6. Promotion enforces deterministic evidence/quality checks before canonical event write.
7. Projection builds read models (GeoJSON, timeline, optional MVT SQL).

Rebuild/replay is done by reprocessing raw documents and rerunning promotion.

// web/src/pages/scholar-gate.tsx
import { useEffect, useState } from "react";
import { Navigate, useParams } from "react-router-dom";

const UUID = /^[0-9a-f-]{36}$/i;

/** Opens the existing Scholar entity page for a UUID or a Wikidata QID. */
export function ScholarEntityGate() {
  const { entityKey = "" } = useParams();
  const [entityId, setEntityId] = useState(UUID.test(entityKey) ? entityKey : "");
  const [missing, setMissing] = useState(false);

  useEffect(() => {
    if (UUID.test(entityKey)) return;
    let cancelled = false;
    async function lookup() {
      const response = await fetch(`/api/v1/entities/by-qid/${encodeURIComponent(entityKey)}`);
      if (!response.ok) {
        if (!cancelled) setMissing(true);
        return;
      }
      const body = (await response.json()) as { entity_id: string };
      if (!cancelled) setEntityId(body.entity_id);
    }
    void lookup();
    return () => {
      cancelled = true;
    };
  }, [entityKey]);

  if (entityId) return <Navigate to={`/entities/${entityId}/overview`} replace />;
  if (missing) return <Navigate to={`/scholar?subject=${encodeURIComponent(entityKey)}`} replace />;
  return <p>Scholar</p>;
}

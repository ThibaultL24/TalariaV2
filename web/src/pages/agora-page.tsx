// web/src/pages/agora-page.tsx
import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { AgoraPanel } from "@/components/explorer/agora-panel";
import { LaneIngestBar } from "@/components/explorer/lane-ingest-bar";
import { Navbar } from "@/components/layout/navbar";
import { EntitySearchBox } from "@/components/search/entity-search-box";
import { usePersonPicker } from "@/hooks/use-person-picker";
import {
  fetchEntityBibliography,
  fetchEntityClaims,
  startAgoraIngest,
  type BibliographyItem,
  type EntityClaim,
} from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import { pollIngestJob } from "@/lib/person-ingest";
import { useExplorerStore } from "@/stores/explorer-store";

export function AgoraPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const { entityId, entityLabel, entityQid, personFilter, setEntity, setSelectedEventId } =
    useExplorerStore();
  const { suggestions, setSearchQuery, searchLoading, selectPerson } = usePersonPicker({
    startLifeIngest: false,
  });
  const agoraLock = useRef<string | null>(null);
  const [claims, setClaims] = useState<EntityClaim[]>([]);
  const [bibliography, setBibliography] = useState<BibliographyItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const subject = entityLabel ?? personFilter;

  useEffect(() => {
    if (!entityId) {
      setClaims([]);
      setBibliography([]);
      return;
    }
    let cancelled = false;
    setLoading(true);
    Promise.all([
      fetchEntityClaims(entityId, { debatesOnly: false, limit: 80 }),
      fetchEntityBibliography(entityId),
    ])
      .then(([nextClaims, biblio]) => {
        if (cancelled) return;
        setClaims(nextClaims);
        setBibliography(biblio.items);
      })
      .catch(() => {
        if (!cancelled) {
          setClaims([]);
          setBibliography([]);
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [entityId, busy]);

  const runAgora = useCallback(async () => {
    if (!subject) return;
    const lockKey = `agora:${subject}:${entityQid ?? ""}`;
    if (agoraLock.current === lockKey) return;
    agoraLock.current = lockKey;
    setBusy(true);
    setError(null);
    try {
      const job = await startAgoraIngest({ subject, qid: entityQid, live: true });
      const bind = (id?: string | null) => {
        if (id) setEntity(id, subject, entityQid);
      };
      bind(job.entity_id);
      const result = await pollIngestJob(job.job_id, (tick) => bind(tick.entity_id));
      bind(result.entity_id);
      if (result.status === "failed") setError(result.error ?? t.loading);
    } catch (err) {
      setError(err instanceof Error ? err.message : t.loading);
    } finally {
      setBusy(false);
    }
  }, [entityQid, setEntity, subject, t.loading]);

  useEffect(() => {
    if (!subject) return;
    void runAgora();
  }, [runAgora, subject]);

  return (
    <div className="app-shell map-shell flex min-h-screen flex-col">
      <Navbar
        center={
          <EntitySearchBox
            suggestions={suggestions}
            onSubmitQuery={setSearchQuery}
            onSelect={selectPerson}
            isLoading={searchLoading}
          />
        }
      />
      <main className="agora-canvas min-h-0 flex-1 overflow-y-auto">
        <section className="hero hero--landing hero--home hero--agora px-4 py-8">
          <div className="mx-auto max-w-3xl">
            <p className="hero__eyebrow">{entityLabel ?? t.agora}</p>
            <h1 className="hero__title hero__title--agora text-4xl">{t.agora}</h1>
            <p className="hero__subtitle">{t.agoraHint}</p>
            {busy ? (
              <p className="mt-3 text-sm text-(--color-text-secondary)">{t.searchInProgress}</p>
            ) : null}
            {error ? <p className="mt-3 text-sm text-red-300">{error}</p> : null}
          </div>
        </section>
        <div className="mx-auto max-w-3xl px-4 pb-12">
          {subject ? (
            <LaneIngestBar lane="agora" busy={busy} onRun={() => void runAgora()} />
          ) : (
            <p className="py-8 text-center text-sm text-(--color-text-secondary)">{t.agoraEmpty}</p>
          )}
          {entityId ? (
            <AgoraPanel
              claims={claims}
              bibliography={bibliography}
              isLoading={loading}
              bibliographyLoading={loading}
              onOpenEvent={(eventId) => {
                setSelectedEventId(eventId);
                navigate("/explorer");
              }}
            />
          ) : null}
        </div>
      </main>
    </div>
  );
}

// web/src/pages/agora-page.tsx
import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { AgoraPanel } from "@/components/explorer/agora-panel";
import { LaneIngestBar } from "@/components/explorer/lane-ingest-bar";
import { IntuitionStanceBar } from "@/components/intuition/intuition-stance-bar";
import { Navbar } from "@/components/layout/navbar";
import { EntitySearchBox } from "@/components/search/entity-search-box";
import { usePersonPicker } from "@/hooks/use-person-picker";
import {
  fetchEntityBibliography,
  fetchEntityClaims,
  searchEntities,
  startAgoraIngest,
  type BibliographyItem,
  type EntityClaim,
} from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import { pollIngestJob } from "@/lib/person-ingest";
import { useExplorerStore } from "@/stores/explorer-store";

export function AgoraPage() {
  const { t, locale } = useI18n();
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

  // If search only set the label (e.g. "Napoléon" vs "Napoleon"), resolve a local entity_id
  // so the claims panel can render without waiting for corpus ingest.
  useEffect(() => {
    if (entityId || !subject) return;
    let cancelled = false;
    void searchEntities(subject, locale)
      .then((items) => {
        if (cancelled) return;
        const local =
          items.find(
            (item) =>
              item.known_locally &&
              item.entity_id &&
              (entityQid ? item.qid === entityQid : true),
          ) ?? items.find((item) => item.known_locally && item.entity_id);
        if (local?.entity_id) {
          setEntity(local.entity_id, local.label ?? subject, local.qid ?? entityQid);
        }
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [entityId, entityQid, locale, setEntity, subject]);

  // Load claims independently of Agora ingest. Tying this to `busy` cancelled
  // in-flight fetches for minutes while corpus ingest ran → empty "Loading agora…".
  useEffect(() => {
    if (!entityId) {
      setClaims([]);
      setBibliography([]);
      return;
    }
    let cancelled = false;
    setLoading(true);
    void (async () => {
      try {
        const [nextClaims, biblio] = await Promise.all([
          fetchEntityClaims(entityId, { debatesOnly: true, limit: 100 }),
          fetchEntityBibliography(entityId),
        ]);
        if (cancelled) return;
        setClaims(nextClaims);
        setBibliography(biblio.items);
      } catch {
        if (!cancelled) {
          setClaims([]);
          setBibliography([]);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [entityId]);

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
      agoraLock.current = null;
    }
  }, [entityQid, setEntity, subject, t.loading]);

  // Background enrichment + entity bind when search only set a label/qid.
  // Does not clear claims; claims load separately once entityId is known.
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
            {entityId ? (
              <div className="mt-4 max-w-md rounded-lg border border-(--color-border-subtle) bg-(--color-bg-elevated)/70 px-3 py-2">
                <IntuitionStanceBar targetKind="person" targetId={entityId} eager />
              </div>
            ) : null}
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

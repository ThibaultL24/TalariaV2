// web/src/features/agora/pages/agora-page.tsx
import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { AgoraPanel } from "@/components/explorer/agora-panel";
import { LaneIngestBar } from "@/components/explorer/lane-ingest-bar";
import { ExperienceNavigation } from "@/components/layout/experience-navigation";
import { ExperienceSubnav } from "@/components/layout/experience-subnav";
import { Navbar } from "@/components/layout/navbar";
import { TalariaSandalMark } from "@/components/brand/talaria-sandal-mark";
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
import { isExperienceView, type AgoraView } from "@/lib/experiences";
import { localizedPersonLabel } from "@/lib/localize-event-copy";
import { useI18n } from "@/lib/i18n";
import { pollIngestJob } from "@/lib/person-ingest";
import { useExplorerStore } from "@/stores/explorer-store";

export function AgoraPage() {
  const { t, locale } = useI18n();
  const navigate = useNavigate();
  const { entityId: routeEntityId, view: routeView } = useParams();
  const section: AgoraView = routeView && isExperienceView("agora", routeView)
    ? routeView as AgoraView
    : "discover";
  const { entityId, entityLabel, entityQid, personFilter, setEntity, setSelectedEventId } =
    useExplorerStore();
  const { suggestions, setSearchQuery, searchLoading, selectPerson } = usePersonPicker({
    startLifeIngest: false,
  });
  const agoraLock = useRef<string | null>(null);
  const routedEntity = useRef<string | undefined>(undefined);
  const [claims, setClaims] = useState<EntityClaim[]>([]);
  const [bibliography, setBibliography] = useState<BibliographyItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const subject = entityLabel ?? personFilter;

  useEffect(() => {
    if (!routeEntityId) return;
    if (routeView && isExperienceView("agora", routeView)) return;
    navigate(`/agora/${routeEntityId}/discover`, { replace: true });
  }, [navigate, routeEntityId, routeView]);

  useEffect(() => {
    if (!routeEntityId || routedEntity.current === routeEntityId) return;
    routedEntity.current = routeEntityId;
    const current = useExplorerStore.getState();
    if (current.entityId !== routeEntityId) setEntity(routeEntityId, current.entityLabel, current.entityQid);
  }, [routeEntityId, setEntity]);

  useEffect(() => {
    if (routeEntityId || !entityId) return;
    navigate(`/agora/${entityId}/discover`, { replace: true });
  }, [entityId, navigate, routeEntityId]);

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

  useEffect(() => {
    if (!subject) return;
    void runAgora();
  }, [runAgora, subject]);

  const activeEntityId = routeEntityId ?? entityId;

  return (
    <div className="app-shell map-shell flex min-h-screen flex-col">
      <Navbar
        center={
          <EntitySearchBox
            suggestions={suggestions}
            onSubmitQuery={setSearchQuery}
            onSelect={(item) => {
              const selectedId = selectPerson(item);
              if (selectedId) navigate(`/agora/${selectedId}/discover`);
            }}
            isLoading={searchLoading}
          />
        }
      />
      <main className="agora-canvas min-h-0 flex-1 overflow-y-auto">
        <ExperienceNavigation entityId={activeEntityId || undefined} compact={Boolean(activeEntityId)} />
        <section className="hero hero--landing hero--home hero--agora hero--agora-photo px-4 py-8">
          <div className="mx-auto max-w-3xl">
            <p className="hero__eyebrow">{localizedPersonLabel(entityLabel, locale) || t.agora}</p>
            <h1 className="hero__title hero__title--agora text-4xl">
              <span className="talaria-heading-with-mark">
                <TalariaSandalMark />
                {t.agora}
              </span>
            </h1>
            <p className="hero__subtitle">{t.agoraHint}</p>
            <p className="hero__subtitle">
              {t.agoraStanceNote}
            </p>
            {busy ? (
              <p className="mt-3 text-sm text-(--color-text-secondary)">{t.searchInProgress}</p>
            ) : null}
            {error ? <p className="mt-3 text-sm text-(--color-trust-low)">{error}</p> : null}
          </div>
        </section>
        {activeEntityId ? <ExperienceSubnav experience="agora" entityId={activeEntityId} /> : null}
        <div className="mx-auto max-w-3xl px-4 pb-12">
          {subject ? (
            <LaneIngestBar lane="agora" busy={busy} onRun={() => void runAgora()} />
          ) : routeEntityId ? null : (
            <p className="py-8 text-center text-sm text-(--color-text-secondary)">{t.agoraEmpty}</p>
          )}
          {entityId ? (
            <AgoraPanel
              section={section}
              claims={claims}
              bibliography={bibliography}
              isLoading={loading}
              bibliographyLoading={loading}
              onOpenEvent={(eventId) => {
                setSelectedEventId(eventId);
                navigate(`/istoria/${entityId}/timeline?event=${encodeURIComponent(eventId)}`);
              }}
            />
          ) : null}
        </div>
      </main>
    </div>
  );
}

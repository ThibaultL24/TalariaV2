// web/src/pages/explorer-page.tsx
import { useEffect, useRef } from "react";
import { Navigate, useNavigate, useSearchParams } from "react-router-dom";
import { IngestProgressBadge } from "@/components/explorer/ingest-progress-badge";
import { Navbar } from "@/components/layout/navbar";
import { EntitySearchBox } from "@/components/search/entity-search-box";
import { usePersonPicker } from "@/hooks/use-person-picker";
import { useI18n } from "@/lib/i18n";
import { useExplorerStore } from "@/stores/explorer-store";

/** Search landing. A resolved person opens the life timeline. */
export function ExplorerPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const entityFromUrl = params.get("entity");
  const entityId = useExplorerStore((state) => state.entityId);
  const started = useRef("");
  const picker = usePersonPicker();
  const subject = params.get("subject") ?? "";

  useEffect(() => {
    if (!subject || started.current === subject) return;
    started.current = subject;
    picker.selectPerson({
      label: subject,
      qid: params.get("qid"),
      known_locally: false,
    });
  }, [subject, params, picker]);

  const destination = entityId || (!subject ? entityFromUrl : null);
  if (destination) {
    const job = picker.jobId ? `?job=${encodeURIComponent(picker.jobId)}` : "";
    return <Navigate to={`/entities/${destination}/overview${job}`} replace />;
  }

  return (
    <div className="v3-shell">
      <Navbar
        center={
          <EntitySearchBox
            suggestions={picker.suggestions}
            onSubmitQuery={picker.setSearchQuery}
            onSelect={(item) => {
              const id = picker.selectPerson(item);
              if (id) navigate(`/entities/${id}/timeline`);
            }}
            isLoading={picker.searchLoading}
          />
        }
      />
      <main className="v3-main">
        <h1>{t.explorer}</h1>
        <p className="v3-note">{subject || t.emptySearch}</p>
        {picker.ingestBusy && (
          <IngestProgressBadge
            phase={picker.ingestPhase}
            currentPage={picker.currentPage}
            timelineEvents={picker.timelineEvents}
            mapPins={picker.mapPins}
            preciseDates={picker.preciseDates}
            preciseCoords={picker.preciseCoords}
            evidenceCount={picker.evidenceCount}
            wikiPages={picker.wikiPages}
            sourcesPending={picker.sourcesPending}
            isRunning={picker.ingestBusy}
            error={picker.error}
          />
        )}
      </main>
    </div>
  );
}

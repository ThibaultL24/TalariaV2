// web/src/components/explorer/ingest-progress-badge.tsx
import { useI18n, type AppMessages } from "@/lib/i18n";

interface IngestProgressBadgeProps {
  phase: string | null;
  currentPage?: string | null;
  timelineEvents: number;
  mapPins: number;
  preciseDates: number;
  preciseCoords: number;
  evidenceCount: number;
  wikiPages: number;
  sourcesPending?: number;
  isRunning: boolean;
  error?: string | null;
}

const PHASE_LABELS: Record<string, { en: string; fr: string }> = {
  queued: { en: "Queued…", fr: "En file…" },
  starting: { en: "Resolving entity…", fr: "Résolution de l’entité…" },
  resolving: { en: "Resolving entity…", fr: "Résolution de l’entité…" },
  wikidata: { en: "Fetching Wikidata…", fr: "Lecture de Wikidata…" },
  wikipedia: { en: "Fetching Wikipedia…", fr: "Lecture de Wikipédia…" },
  extracting: { en: "Extracting events…", fr: "Extraction des faits…" },
  core_extract: { en: "Extracting events…", fr: "Extraction des faits…" },
  wdqs: { en: "Querying WDQS…", fr: "Requête WDQS…" },
  following_links: { en: "Following links…", fr: "Suivi des liens…" },
  grounding: { en: "Grounding places…", fr: "Ancrage des lieux…" },
  corpus_enrichment: { en: "Enriching with sources…", fr: "Enrichissement des sources…" },
  persisting: { en: "Saving…", fr: "Enregistrement…" },
  done: { en: "Complete", fr: "Terminé" },
  failed: { en: "Failed", fr: "Échec" },
};

function phaseLabel(phase: string | null, t: AppMessages, locale: "en" | "fr"): string {
  if (!phase) return t.loadingMap;
  return PHASE_LABELS[phase]?.[locale] ?? t.searchInProgress;
}

function truncateTitle(title: string | null | undefined, maxLen = 28): string | null {
  if (!title) return null;
  if (title.length <= maxLen) return title;
  return title.slice(0, maxLen - 1) + "…";
}

export function IngestProgressBadge({
  phase,
  currentPage,
  timelineEvents,
  mapPins,
  preciseDates,
  preciseCoords,
  evidenceCount,
  wikiPages,
  sourcesPending,
  isRunning,
  error,
}: IngestProgressBadgeProps) {
  const { t, locale } = useI18n();

  if (error) {
    return (
      <div className="pointer-events-none absolute top-3 left-1/2 z-10 -translate-x-1/2 rounded-lg border border-red-500/30 bg-red-950/80 px-3 py-1.5 text-[11px] text-red-200 backdrop-blur-sm">
        {error}
      </div>
    );
  }

  const hasEvents = timelineEvents > 0 || mapPins > 0;
  const countsText = hasEvents ? t.ingestEventsPins(timelineEvents, mapPins) : null;

  const precisionText =
    hasEvents && (preciseDates > 0 || preciseCoords > 0)
      ? t.ingestDatedLocated(preciseDates, preciseCoords)
      : null;

  const sourcesText =
    wikiPages > 0 || evidenceCount > 0
      ? t.ingestPagesSources(wikiPages, evidenceCount)
      : null;

  const truncatedPage = truncateTitle(currentPage);
  const showFollowProgress = (sourcesPending ?? 0) > 0 && phase === "following_links";

  return (
    <div className="pointer-events-none absolute top-3 left-1/2 z-10 -translate-x-1/2 flex flex-col items-center gap-1">
      {isRunning && (
        <div className="flex items-center gap-2 rounded-full border border-(--map-panel-border) bg-(--color-bg-elevated)/85 px-3 py-1 text-[11px] text-(--color-text-secondary) backdrop-blur-sm">
          <span className="relative flex h-2 w-2">
            <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-blue-400 opacity-75" />
            <span className="relative inline-flex h-2 w-2 rounded-full bg-blue-500" />
          </span>
          <span className="flex flex-col items-center">
            <span>{phaseLabel(phase, t, locale)}</span>
            {truncatedPage && (
              <span className="text-[9px] text-(--color-text-muted)/70 max-w-[180px] truncate">
                {truncatedPage}
              </span>
            )}
          </span>
        </div>
      )}
      {countsText && (
        <div className="rounded-full border border-(--map-panel-border) bg-(--color-bg-elevated)/75 px-2.5 py-0.5 text-[10px] text-(--color-text-muted) backdrop-blur-sm">
          {countsText}
        </div>
      )}
      {(precisionText || sourcesText || showFollowProgress) && (
        <div className="rounded-full border border-(--map-panel-border) bg-(--color-bg-elevated)/65 px-2 py-0.5 text-[9px] text-(--color-text-muted)/80 backdrop-blur-sm">
          {[
            precisionText,
            sourcesText,
            showFollowProgress ? t.ingestQueuedPages(sourcesPending ?? 0) : null,
          ].filter(Boolean).join(" · ")}
        </div>
      )}
    </div>
  );
}

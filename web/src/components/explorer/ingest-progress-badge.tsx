// web/src/components/explorer/ingest-progress-badge.tsx
import { useI18n, type AppMessages } from "@/lib/i18n";

interface IngestProgressBadgeProps {
  phase: string | null;
  timelineEvents: number;
  mapPins: number;
  wikiPages: number;
  isRunning: boolean;
  error?: string | null;
}

const PHASE_LABELS: Record<string, string> = {
  queued: "Queued…",
  starting: "Resolving entity…",
  resolving: "Resolving entity…",
  collecting: "Collecting sources…",
  extracting: "Extracting events…",
  persisting: "Saving…",
  done: "Complete",
  failed: "Failed",
};

function phaseLabel(phase: string | null, t: AppMessages): string {
  if (!phase) return t.loadingMap;
  return PHASE_LABELS[phase] ?? t.searchInProgress;
}

export function IngestProgressBadge({
  phase,
  timelineEvents,
  mapPins,
  wikiPages,
  isRunning,
  error,
}: IngestProgressBadgeProps) {
  const { t } = useI18n();

  if (error) {
    return (
      <div className="pointer-events-none absolute top-3 left-1/2 z-10 -translate-x-1/2 rounded-lg border border-red-500/30 bg-red-950/80 px-3 py-1.5 text-[11px] text-red-200 backdrop-blur-sm">
        {error}
      </div>
    );
  }

  const countsText =
    timelineEvents > 0 || mapPins > 0
      ? `${timelineEvents} événements · ${mapPins} pins`
      : null;

  const sourcesText = wikiPages > 0 ? `${wikiPages} pages` : null;

  return (
    <div className="pointer-events-none absolute top-3 left-1/2 z-10 -translate-x-1/2 flex flex-col items-center gap-1">
      {isRunning && (
        <div className="flex items-center gap-2 rounded-full border border-(--map-panel-border) bg-(--color-bg-elevated)/85 px-3 py-1 text-[11px] text-(--color-text-secondary) backdrop-blur-sm">
          <span className="relative flex h-2 w-2">
            <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-blue-400 opacity-75" />
            <span className="relative inline-flex h-2 w-2 rounded-full bg-blue-500" />
          </span>
          <span>{phaseLabel(phase, t)}</span>
        </div>
      )}
      {(countsText || sourcesText) && (
        <div className="rounded-full border border-(--map-panel-border) bg-(--color-bg-elevated)/75 px-2.5 py-0.5 text-[10px] text-(--color-text-muted) backdrop-blur-sm">
          {[countsText, sourcesText].filter(Boolean).join(" · ")}
        </div>
      )}
    </div>
  );
}

// web/src/components/map/explorer-map-timeline-bar.tsx
import { useI18n } from "@/lib/i18n";

interface ExplorerMapTimelineBarProps {
  bounds: { min: number; max: number };
  untilYear: number;
  onUntilYearChange: (year: number) => void;
  visibleCount: number;
  totalCount: number;
  yearHistogram?: readonly { year: number; count: number }[];
}

function buildWaveformBuckets(
  bounds: { min: number; max: number },
  histogram: readonly { year: number; count: number }[] | undefined,
  bucketCount = 48,
): number[] {
  const span = Math.max(bounds.max - bounds.min, 1);
  const buckets = Array.from({ length: bucketCount }, () => 0);
  if (!histogram?.length) {
    return buckets.map((_, index) => 0.15 + 0.55 * Math.sin((index / bucketCount) * Math.PI));
  }
  for (const { year, count } of histogram) {
    const t = (year - bounds.min) / span;
    const idx = Math.min(bucketCount - 1, Math.max(0, Math.floor(t * bucketCount)));
    buckets[idx] += count;
  }
  const max = Math.max(...buckets, 1);
  return buckets.map((n) => (n > 0 ? 0.2 + (n / max) * 0.8 : 0.08));
}

export function ExplorerMapTimelineBar({
  bounds,
  untilYear,
  onUntilYearChange,
  visibleCount,
  totalCount,
  yearHistogram,
}: ExplorerMapTimelineBarProps) {
  const { t } = useI18n();
  const waveform = buildWaveformBuckets(bounds, yearHistogram);
  const span = Math.max(bounds.max - bounds.min, 1);
  const fillWidth = ((untilYear - bounds.min) / span) * 100;

  return (
    <div
      className="surface-nav nebula-panel pointer-events-auto absolute right-3 bottom-3 left-3 z-10 px-4 py-3"
      role="region"
      aria-label={t.untilYear}
    >
      <div className="flex items-center justify-between gap-2 text-[11px]">
        <span className="nebula-section-kicker text-[10px] font-semibold uppercase">
          {t.untilYear} {untilYear}
        </span>
        <span className="tabular-nums text-(--color-text-muted)">
          {t.eventsVisible(visibleCount, totalCount)}
        </span>
      </div>

      <div className="nebula-waveform mt-3" aria-hidden>
        {waveform.map((height, index) => {
          const bucketYear =
            bounds.min + Math.floor((index / waveform.length) * (bounds.max - bounds.min));
          const inRange = bucketYear <= untilYear;
          return (
            <div
              key={index}
              className={`nebula-waveform__bar ${inRange ? "is-active" : ""}`}
              style={{ height: `${Math.round(height * 100)}%` }}
            />
          );
        })}
      </div>

      <div className="relative mt-3">
        <div className="nebula-range-track" aria-hidden>
          <div className="nebula-range-track__fill" style={{ left: 0, width: `${fillWidth}%` }} />
        </div>
        <input
          type="range"
          min={bounds.min}
          max={bounds.max}
          value={untilYear}
          onChange={(event) => onUntilYearChange(Number(event.target.value))}
          className="nebula-range mt-0"
          aria-valuemin={bounds.min}
          aria-valuemax={bounds.max}
          aria-valuenow={untilYear}
          aria-label={`${t.untilYear} ${untilYear}`}
        />
      </div>

      <div className="mt-1 flex justify-between text-[10px] tabular-nums text-(--color-text-muted)">
        <span>{bounds.min}</span>
        <span>{bounds.max}</span>
      </div>
    </div>
  );
}

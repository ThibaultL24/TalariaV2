// src/components/map/explorer-map-timeline-bar.tsx

import { strings } from "@/lib/strings";

interface ExplorerMapTimelineBarProps {
  bounds: { min: number; max: number };
  range: { min: number; max: number };
  onRangeChange: (range: { min: number; max: number }) => void;
  visibleCount: number;
  totalCount: number;
  yearHistogram?: readonly { year: number; count: number }[];
}

function buildWaveformBuckets(
  bounds: { min: number; max: number },
  histogram: readonly { year: number; count: number }[] | undefined,
  bucketCount = 24
): number[] {
  const span = Math.max(bounds.max - bounds.min, 1);
  const buckets = Array.from({ length: bucketCount }, () => 0);

  if (!histogram?.length) {
    return buckets.map((_, i) => 0.15 + 0.55 * Math.sin((i / bucketCount) * Math.PI));
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
  range,
  onRangeChange,
  visibleCount,
  totalCount,
  yearHistogram,
}: ExplorerMapTimelineBarProps) {
  const waveform = buildWaveformBuckets(bounds, yearHistogram);
  const span = Math.max(bounds.max - bounds.min, 1);
  const fillLeft = ((range.min - bounds.min) / span) * 100;
  const fillWidth = ((range.max - range.min) / span) * 100;

  return (
    <div
      className="surface-nav nebula-panel pointer-events-auto absolute bottom-3 left-3 right-3 z-10 max-w-lg px-3 py-3 sm:left-3 sm:right-auto sm:w-[min(100%,26rem)]"
      role="region"
      aria-label={strings.explorerMapTimelineBar}
    >
      <div className="flex items-center justify-between gap-2 text-[11px]">
        <span className="nebula-section-kicker text-[10px] font-semibold uppercase">
          {strings.explorerMapTimelineBar}
        </span>
        <span className="tabular-nums text-(--color-text-muted)">
          {range.min} → {range.max} · {visibleCount}/{totalCount}
        </span>
      </div>

      <div className="nebula-waveform mt-3" aria-hidden>
        {waveform.map((height, index) => {
          const bucketYear =
            bounds.min + Math.floor((index / waveform.length) * (bounds.max - bounds.min));
          const inRange = bucketYear >= range.min && bucketYear <= range.max;
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
          <div
            className="nebula-range-track__fill"
            style={{ left: `${fillLeft}%`, width: `${fillWidth}%` }}
          />
        </div>
        <input
          type="range"
          min={bounds.min}
          max={bounds.max}
          value={range.min}
          onChange={(e) =>
            onRangeChange({ min: Math.min(Number(e.target.value), range.max), max: range.max })
          }
          className="nebula-range mt-0"
          aria-label={strings.explorerPeriodFrom}
        />
        <input
          type="range"
          min={bounds.min}
          max={bounds.max}
          value={range.max}
          onChange={(e) =>
            onRangeChange({ min: range.min, max: Math.max(Number(e.target.value), range.min) })
          }
          className="nebula-range -mt-1.5"
          aria-label={strings.explorerPeriodTo}
        />
      </div>
    </div>
  );
}

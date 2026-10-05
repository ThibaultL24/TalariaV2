// web/src/features/entity/timeline-histography-brush.tsx
import { useRef, type PointerEvent as ReactPointerEvent } from "react";

export type YearWindow = [number, number];

interface HistographyBrushProps {
  bounds: YearWindow;
  window: YearWindow;
  histogram: number[];
  onChange: (next: YearWindow) => void;
  onCommit: () => void;
}

function clampWindow(window: YearWindow, bounds: YearWindow): YearWindow {
  const full = Math.max(1, bounds[1] - bounds[0]);
  const span = Math.min(full, Math.max(1, window[1] - window[0]));
  const start = Math.max(bounds[0], Math.min(bounds[1] - span, window[0]));
  return [start, start + span];
}

export function HistographyBrush({ bounds, window, histogram, onChange, onCommit }: HistographyBrushProps) {
  const trackRef = useRef<HTMLDivElement>(null);
  const drag = useRef<
    | { mode: "move" | "start" | "end"; originX: number; originWindow: YearWindow }
    | undefined
  >(undefined);
  const fullSpan = Math.max(1, bounds[1] - bounds[0]);

  function yearsFromClientX(clientX: number): number {
    const track = trackRef.current;
    if (!track) return bounds[0];
    const rect = track.getBoundingClientRect();
    const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    return bounds[0] + ratio * fullSpan;
  }

  function onPointerDown(
    mode: "move" | "start" | "end",
    event: ReactPointerEvent<HTMLElement>,
  ) {
    event.preventDefault();
    event.stopPropagation();
    drag.current = { mode, originX: event.clientX, originWindow: window };
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function onPointerMove(event: ReactPointerEvent<HTMLElement>) {
    const gesture = drag.current;
    if (!gesture) return;
    const track = trackRef.current;
    if (!track) return;
    const rect = track.getBoundingClientRect();
    const deltaYears = ((event.clientX - gesture.originX) / rect.width) * fullSpan;
    const [from, to] = gesture.originWindow;
    if (gesture.mode === "move") {
      onChange(clampWindow([from + deltaYears, to + deltaYears], bounds));
      return;
    }
    if (gesture.mode === "start") {
      onChange(clampWindow([from + deltaYears, to], bounds));
      return;
    }
    onChange(clampWindow([from, to + deltaYears], bounds));
  }

  function onPointerUp(event: ReactPointerEvent<HTMLElement>) {
    if (!drag.current) return;
    drag.current = undefined;
    event.currentTarget.releasePointerCapture(event.pointerId);
    onCommit();
  }

  function jumpTo(event: ReactPointerEvent<HTMLDivElement>) {
    if (drag.current) return;
    const center = yearsFromClientX(event.clientX);
    const span = window[1] - window[0];
    onChange(clampWindow([center - span / 2, center + span / 2], bounds));
    onCommit();
  }

  const leftPct = ((window[0] - bounds[0]) / fullSpan) * 100;
  const widthPct = ((window[1] - window[0]) / fullSpan) * 100;
  const maxBin = Math.max(1, ...histogram);

  return (
    <div className="v3-histo-brush" ref={trackRef} onClick={jumpTo} role="presentation">
      <div className="v3-histo-density" aria-hidden="true">
        {histogram.map((count, index) => (
          <span key={index} style={{ height: `${Math.max(4, (count / maxBin) * 100)}%` }} />
        ))}
      </div>
      <div
        className="v3-histo-window"
        style={{ left: `${leftPct}%`, width: `${widthPct}%` }}
        onPointerDown={(e) => onPointerDown("move", e)}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
        role="slider"
        aria-label="Visible period"
        aria-valuemin={bounds[0]}
        aria-valuemax={bounds[1]}
        aria-valuenow={Math.round(window[0])}
      >
        <button
          type="button"
          className="v3-histo-handle v3-histo-handle--start"
          aria-label="Adjust period start"
          onPointerDown={(e) => onPointerDown("start", e)}
        />
        <button
          type="button"
          className="v3-histo-handle v3-histo-handle--end"
          aria-label="Adjust period end"
          onPointerDown={(e) => onPointerDown("end", e)}
        />
      </div>
    </div>
  );
}

export function lifePhasePresets(bounds: YearWindow): { id: string; label: string; window: YearWindow }[] {
  const [start, end] = bounds;
  const span = Math.max(1, end - start);
  const third = span / 3;
  return [
    { id: "full", label: "Full life", window: bounds },
    { id: "early", label: "Early years", window: [start, start + third] },
    { id: "middle", label: "Middle years", window: [start + third, start + 2 * third] },
    { id: "late", label: "Later years", window: [start + 2 * third, end] },
  ];
}

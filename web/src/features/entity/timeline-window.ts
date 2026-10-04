// web/src/features/entity/timeline-window.ts

/** Keep a window inside the life span. Width stays at least one year. */
export function clampWindow(
  lifeFrom: number,
  lifeTo: number,
  from: number,
  to: number,
): [number, number] {
  const span = Math.max(1, lifeTo - lifeFrom);
  const width = Math.max(1, Math.min(span, to - from));
  let start = from;
  if (start < lifeFrom) start = lifeFrom;
  if (start + width > lifeTo) start = lifeTo - width;
  return [Math.round(start), Math.round(start + width)];
}

/** Slide the current span so `year` sits in the middle, then clamp to the life. */
export function windowCenteredOn(
  lifeFrom: number,
  lifeTo: number,
  span: number,
  year: number,
): [number, number] {
  const width = Math.max(1, Math.min(Math.max(1, lifeTo - lifeFrom), span));
  const start = year - width / 2;
  return clampWindow(lifeFrom, lifeTo, start, start + width);
}

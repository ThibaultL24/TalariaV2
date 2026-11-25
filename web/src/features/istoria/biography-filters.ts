// web/src/features/istoria/biography-filters.ts

const FILTER_KEYS = ["from", "to", "types", "group"] as const;

/** The frieze zooms locally, so its request keeps the chapter but not the year window. */
export function biographyFilters(params: URLSearchParams, view: string): string {
  const filters = new URLSearchParams();
  for (const key of FILTER_KEYS) {
    const value = params.get(key);
    const timelineOwnsYears = view === "timeline" && (key === "from" || key === "to");
    if (value && !timelineOwnsYears) filters.set(key, value);
  }
  return filters.toString();
}

// web/src/lib/source-labels.ts
const SOURCE_LABELS: Record<string, string> = {
  hal: "HAL",
  persee: "Persée",
  gallica: "Gallica",
  theses_fr: "theses.fr",
  open_alex: "OpenAlex",
  wikipedia: "Wikipedia",
  wikidata: "Wikidata",
  open_library: "Open Library",
  internet_archive: "Internet Archive",
  europeana: "Europeana",
};

export function sourceSystemLabel(source: string | null | undefined): string {
  if (!source) return "Source";
  const key = source.trim().toLowerCase();
  return SOURCE_LABELS[key] ?? source.replace(/_/g, " ");
}

export function sourceKindBadgeClass(source: string | null | undefined): string {
  const key = (source ?? "").trim().toLowerCase();
  switch (key) {
    case "hal":
      return "bg-orange-500/15 text-orange-200";
    case "persee":
      return "bg-emerald-500/15 text-emerald-200";
    case "gallica":
      return "bg-amber-500/15 text-amber-200";
    case "theses_fr":
      return "bg-violet-500/15 text-violet-200";
    case "open_alex":
      return "bg-sky-500/15 text-sky-200";
    default:
      return "bg-white/10 text-(--color-text-muted)";
  }
}

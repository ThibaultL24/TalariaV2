// web/src/lib/experiences.ts

export type ExperienceId = "istoria" | "poreia" | "agora";
export type LifeExperience = "istoria" | "poreia";

export const ISTORIA_VIEWS = ["overview", "timeline", "map", "sources"] as const;
export const POREIA_VIEWS = ["map", "heritage", "itinerary", "now"] as const;
export const AGORA_VIEWS = ["discover", "theses", "arguments", "sources"] as const;

export type IstoriaView = (typeof ISTORIA_VIEWS)[number];
export type PoreiaView = (typeof POREIA_VIEWS)[number];
export type AgoraView = (typeof AGORA_VIEWS)[number];

const VIEWS: Record<ExperienceId, readonly string[]> = {
  istoria: ISTORIA_VIEWS,
  poreia: POREIA_VIEWS,
  agora: AGORA_VIEWS,
};

const TAB_LABELS: Record<ExperienceId, Record<string, { fr: string; en: string }>> = {
  istoria: {
    overview: { fr: "Aperçu", en: "Overview" },
    timeline: { fr: "Chronologie", en: "Timeline" },
    map: { fr: "Carte de vie", en: "Life map" },
    sources: { fr: "Sources", en: "Sources" },
  },
  poreia: {
    map: { fr: "Carte touristique", en: "Tourist map" },
    heritage: { fr: "Lieux & monuments", en: "Places & monuments" },
    itinerary: { fr: "Parcours", en: "Itinerary" },
    now: { fr: "En ce moment", en: "Right now" },
  },
  agora: {
    discover: { fr: "Découvrir", en: "Discover" },
    theses: { fr: "Thèses", en: "Theses" },
    arguments: { fr: "Arguments", en: "Arguments" },
    sources: { fr: "Sources", en: "Sources" },
  },
};

export function defaultExperienceView(experience: ExperienceId): string {
  if (experience === "poreia") return "map";
  if (experience === "agora") return "discover";
  return "overview";
}

export function isExperienceView(experience: ExperienceId, view: string): boolean {
  return VIEWS[experience].includes(view);
}

export function experienceTabs(experience: ExperienceId, locale: "fr" | "en") {
  return VIEWS[experience].map((id) => ({
    id,
    label: TAB_LABELS[experience][id][locale],
  }));
}

export function entityIdFromPathname(pathname: string): string | undefined {
  const [experience, entityId] = pathname.split("/").filter(Boolean);
  if (experience !== "istoria" && experience !== "poreia" && experience !== "agora") return undefined;
  if (!entityId) return undefined;
  return decodeURIComponent(entityId);
}

export function experienceHref(experience: ExperienceId, entityId?: string): string {
  if (!entityId) return `/${experience}`;
  const id = encodeURIComponent(entityId);
  if (experience === "agora") return `/agora/${id}/${defaultExperienceView(experience)}`;
  return `/${experience}/${id}/${defaultExperienceView(experience)}`;
}

export function experienceSectionHref(
  experience: ExperienceId,
  entityId: string,
  view: string,
  search = "",
): string {
  const params = search.startsWith("?") ? search.slice(1) : search;
  return `/${experience}/${encodeURIComponent(entityId)}/${view}${params ? `?${params}` : ""}`;
}

/** Old `/entities/:id/:view?lens=visit` URLs. Drops `lens`, keeps the other filters. */
export function legacyEntityTarget(entityId: string, view: string | undefined, search: string): string {
  const params = new URLSearchParams(search.startsWith("?") ? search.slice(1) : search);
  const experience: LifeExperience = params.get("lens") === "visit" ? "poreia" : "istoria";
  params.delete("lens");
  const nextView = view && isExperienceView(experience, view) ? view : defaultExperienceView(experience);
  const query = params.toString();
  return experienceSectionHref(experience, entityId, nextView, query);
}

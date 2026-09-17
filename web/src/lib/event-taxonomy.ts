// web/src/lib/event-taxonomy.ts
/** Objective category families for Explorer filters. Empty selection = show all. */

export const EVENT_TYPE_OPTIONS = [
  { key: "birth", label: "Birth", family: "biography" },
  { key: "death", label: "Death", family: "biography" },
  { key: "marriage", label: "Marriage", family: "biography" },
  { key: "divorce", label: "Divorce", family: "biography" },
  { key: "education", label: "Education", family: "biography" },
  { key: "employment", label: "Employment", family: "biography" },
  { key: "relocation", label: "Relocation", family: "biography" },
  { key: "travel", label: "Travel", family: "biography" },
  { key: "residence", label: "Residence", family: "biography" },
  { key: "exile", label: "Exile", family: "biography" },
  { key: "imprisonment", label: "Imprisonment", family: "biography" },
  { key: "battle", label: "Battle", family: "biography" },
  { key: "diplomatic", label: "Diplomatic", family: "biography" },
  { key: "meeting", label: "Meeting", family: "biography" },
  { key: "office", label: "Office", family: "biography" },
  { key: "speech", label: "Speech", family: "biography" },
  { key: "award", label: "Award", family: "biography" },
  { key: "publication", label: "Publication", family: "work" },
  { key: "creation", label: "Creation", family: "work" },
  { key: "discovery", label: "Discovery", family: "work" },
  { key: "anecdote", label: "Anecdote", family: "narrative" },
  { key: "statue", label: "Statue", family: "legacy" },
  { key: "museum", label: "Museum", family: "legacy" },
  { key: "street_naming", label: "Street naming", family: "legacy" },
  { key: "memorial", label: "Memorial", family: "legacy" },
  { key: "life_event", label: "Life event", family: "other" },
  { key: "historical_fact", label: "Historical fact", family: "other" },
] as const;

export const EPISTEMIC_STATUS_OPTIONS = [
  { key: "established", label: "Established fact" },
  { key: "attested", label: "Attested" },
  { key: "uncertain", label: "Uncertain" },
  { key: "theory", label: "Theory" },
  { key: "rumor", label: "Rumor" },
] as const;

export type EpistemicStatus = (typeof EPISTEMIC_STATUS_OPTIONS)[number]["key"];

export function eventTypeLabel(eventType: string, locale: "en" | "fr" = "en"): string {
  const labels =
    locale === "fr"
      ? {
          birth: "Naissance",
          death: "Mort",
          marriage: "Mariage",
          divorce: "Divorce",
          education: "Études",
          employment: "Emploi",
          relocation: "Déménagement",
          travel: "Voyage",
          residence: "Résidence",
          exile: "Exil",
          imprisonment: "Emprisonnement",
          battle: "Bataille",
          siege: "Siège",
          diplomatic: "Diplomatie",
          meeting: "Rencontre",
          office: "Charge",
          speech: "Discours",
          award: "Distinction",
          publication: "Publication",
          creation: "Création",
          discovery: "Découverte",
          anecdote: "Anecdote",
          statue: "Statue",
          museum: "Musée",
          street_naming: "Odonymie",
          memorial: "Mémorial",
          life_event: "Fait de vie",
          historical_fact: "Fait historique",
          trial: "Procès",
          legal_event: "Fait juridique",
          political_event: "Fait politique",
          health_event: "Santé",
          financial_event: "Argent",
          censorship: "Censure",
        }
      : {
          birth: "Birth",
          death: "Death",
          marriage: "Marriage",
          divorce: "Divorce",
          education: "Education",
          employment: "Employment",
          relocation: "Relocation",
          travel: "Travel",
          residence: "Residence",
          exile: "Exile",
          imprisonment: "Imprisonment",
          battle: "Battle",
          siege: "Siege",
          diplomatic: "Diplomatic",
          meeting: "Meeting",
          office: "Office",
          speech: "Speech",
          award: "Award",
          publication: "Publication",
          creation: "Creation",
          discovery: "Discovery",
          anecdote: "Anecdote",
          statue: "Statue",
          museum: "Museum",
          street_naming: "Street naming",
          memorial: "Memorial",
          life_event: "Life event",
          historical_fact: "Historical fact",
          trial: "Trial",
          legal_event: "Legal event",
          political_event: "Political event",
          health_event: "Health",
          financial_event: "Money",
          censorship: "Censorship",
        };
  return labels[eventType as keyof typeof labels] ?? eventType.replace(/_/g, " ");
}

export function epistemicStatusLabel(status: string, locale: "en" | "fr" = "en"): string {
  const labels =
    locale === "fr"
      ? {
          established: "Établi",
          attested: "Attesté",
          uncertain: "Incertain",
          theory: "Théorie",
          rumor: "Rumeur",
        }
      : {
          established: "Established fact",
          attested: "Attested",
          uncertain: "Uncertain",
          theory: "Theory",
          rumor: "Rumor",
        };
  return labels[status as keyof typeof labels] ?? status.replace(/_/g, " ");
}

export function epistemicBadgeClass(status: string): string {
  switch (status) {
    case "established":
      return "bg-emerald-500/15 text-emerald-300";
    case "attested":
      return "bg-sky-500/15 text-sky-300";
    case "uncertain":
      return "bg-amber-500/15 text-amber-200";
    case "theory":
      return "bg-violet-500/15 text-violet-200";
    case "rumor":
      return "bg-rose-500/15 text-rose-300";
    default:
      return "bg-white/10 text-(--color-text-muted)";
  }
}

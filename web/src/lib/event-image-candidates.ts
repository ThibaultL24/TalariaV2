// web/src/lib/event-image-candidates.ts
export type EventImageCandidateKind = "event" | "place" | "person";

export interface EventImageCandidate {
  title: string;
  lang: string;
  kind: EventImageCandidateKind;
}

export interface EventImageCandidateInput {
  eventType: string;
  personLabel?: string | null;
  placeLabel?: string | null;
  primaryObject?: string | null;
  sourcePageTitles?: string[];
  sourceRefs?: Array<{ page_title?: string | null; language?: string | null }>;
  wikipediaUrl?: string | null;
  defaultLang?: string;
}

const EVENT_TITLE_RE =
  /^(battle|siege|treaty|treaties|action|campaign|capture|convention|peace)\b/i;

function normalizeTitle(raw: string): string {
  return raw.replace(/_/g, " ").trim();
}

function isPersonTitle(title: string, personLabel?: string | null): boolean {
  if (!personLabel) return false;
  return title.trim().toLowerCase() === personLabel.trim().toLowerCase();
}

function looksLikeEventTitle(title: string): boolean {
  return EVENT_TITLE_RE.test(title.trim());
}

/** Ordered Wikipedia titles to try for an event hero image. */
export function buildEventImageCandidates(
  input: EventImageCandidateInput,
): EventImageCandidate[] {
  const lang = (input.defaultLang ?? "en").trim() || "en";
  const out: EventImageCandidate[] = [];
  const seen = new Set<string>();

  function push(
    titleRaw: string | null | undefined,
    kind: EventImageCandidateKind,
    pageLang = lang,
  ) {
    if (!titleRaw) return;
    const title = normalizeTitle(titleRaw);
    if (title.length < 2) return;
    const key = `${pageLang}:${title.toLowerCase()}`;
    if (seen.has(key)) return;
    seen.add(key);
    out.push({ title, lang: pageLang, kind });
  }

  if (input.primaryObject) push(input.primaryObject, "event");

  for (const ref of input.sourceRefs ?? []) {
    const title = ref.page_title;
    if (!title) continue;
    if (isPersonTitle(title, input.personLabel) && !looksLikeEventTitle(title)) continue;
    push(title, "event", ref.language ?? lang);
  }

  for (const title of input.sourcePageTitles ?? []) {
    if (isPersonTitle(title, input.personLabel) && !looksLikeEventTitle(title)) continue;
    push(title, "event");
  }

  if (input.placeLabel) push(input.placeLabel, "place");

  const type = input.eventType.toLowerCase();
  if (type === "birth" || type === "death") {
    if (input.personLabel) push(input.personLabel, "person");
  }

  return out;
}

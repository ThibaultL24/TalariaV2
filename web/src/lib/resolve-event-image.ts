// web/src/lib/resolve-event-image.ts
import {
  buildEventImageCandidates,
  type EventImageCandidateInput,
  type EventImageCandidateKind,
} from "@/lib/event-image-candidates";
import { fetchWikipediaPageSummary } from "@/lib/wikipedia";

export interface ResolvedEventImage {
  url: string;
  pageTitle: string;
  pageUrl: string;
  kind: EventImageCandidateKind;
}

/** Resolve the first Wikipedia thumbnail for an event, in candidate priority order. */
export async function resolveEventImage(
  input: EventImageCandidateInput,
): Promise<ResolvedEventImage | null> {
  const candidates = buildEventImageCandidates(input);
  for (const candidate of candidates) {
    const summary = await fetchWikipediaPageSummary(candidate.lang, candidate.title);
    if (!summary?.thumbnailSource) continue;
    return {
      url: summary.thumbnailSource,
      pageTitle: summary.title ?? candidate.title,
      pageUrl:
        summary.url ??
        `https://${candidate.lang}.wikipedia.org/wiki/${candidate.title.replace(/ /g, "_")}`,
      kind: candidate.kind,
    };
  }
  return null;
}

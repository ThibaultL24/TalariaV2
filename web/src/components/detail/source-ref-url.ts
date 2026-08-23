// web/src/components/detail/source-ref-url.ts
import type { EventSourceRef } from "@/lib/api";

function isHttpUrl(value: string | undefined | null): value is string {
  return Boolean(value && /^https?:\/\//i.test(value.trim()));
}

/** Prefer API URL; else build Wikipedia article/revision from page_title + oldid. */
export function resolveSourceRefHref(
  ref: EventSourceRef,
  defaultLang = "en",
): string | null {
  if (isHttpUrl(ref.url)) return ref.url.trim();
  if (isHttpUrl(ref.source_url)) return ref.source_url.trim();
  if (isHttpUrl(ref.revision_url)) return ref.revision_url.trim();
  if (isHttpUrl(ref.wikipedia_url)) return ref.wikipedia_url.trim();
  if (isHttpUrl(ref.page_url)) return ref.page_url.trim();
  return buildWikipediaSourceUrl(ref, defaultLang);
}

export function buildWikipediaSourceUrl(
  ref: EventSourceRef,
  defaultLang = "en",
): string | null {
  const title = (ref.page_title ?? ref.source_page_title)?.trim();
  if (!title) return null;
  const wikiLang = ref.language ?? defaultLang;
  const slug = title.replace(/\s+/g, "_");
  const oldid = ref.oldid ?? ref.revision_id;
  if (oldid != null) {
    return `https://${wikiLang}.wikipedia.org/w/index.php?title=${encodeURIComponent(slug)}&oldid=${oldid}`;
  }
  return `https://${wikiLang}.wikipedia.org/wiki/${encodeURIComponent(slug)}`;
}

function wikipediaSectionSlug(section: string): string {
  return section.trim().replace(/\s+/g, "_");
}

function textFragmentFromQuote(quote: string): string | null {
  const words = quote
    .replace(/\s+/g, " ")
    .trim()
    .split(" ")
    .filter((word) => word.length > 0)
    .slice(0, 8);
  if (words.join(" ").length < 12) return null;
  return encodeURIComponent(words.join(" "));
}

/** Prefer the cited paragraph: section hash + browser text fragment when a quote exists. */
export function resolveSourceParagraphHref(
  ref: EventSourceRef,
  defaultLang = "en",
): string | null {
  const base = resolveSourceRefHref(ref, defaultLang);
  if (!base) return null;
  const section = ref.section_title?.trim();
  const fragment = textFragmentFromQuote(ref.snippet ?? ref.quote ?? "");
  const hashParts: string[] = [];
  if (section) hashParts.push(wikipediaSectionSlug(section));
  if (fragment) hashParts.push(`:~:text=${fragment}`);
  if (hashParts.length === 0) return base;
  const withoutHash = base.split("#")[0];
  return `${withoutHash}#${hashParts.join("")}`;
}

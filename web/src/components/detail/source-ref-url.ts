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
  const host = `${wikiLang}.wikipedia.org`;
  if (oldid != null) {
    return `https://${host}/w/index.php?title=${encodeURIComponent(slug)}&oldid=${oldid}`;
  }
  return `https://${host}/wiki/${encodeURIComponent(slug)}`;
}

function wikipediaSectionSlug(section: string): string {
  return section.trim().replace(/\s+/g, "_");
}

/** Labels stored for our own evidence rows. They are not headings on the source page. */
function isInternalSection(section: string): boolean {
  const value = section.trim().toLowerCase();
  return value === "evidence" || /^sentence \d+$/.test(value) || /^adjacent\b/.test(value);
}

function textFragmentFromQuote(quote: string): string | null {
  if ((quote.match(/\|/g) ?? []).length >= 2) return null;
  const words = quote
    .replace(/\s+/g, " ")
    .trim()
    .split(" ")
    .filter((word) => word.length > 0)
    .slice(0, 8);
  if (words.join(" ").length < 20) return null;
  return encodeURIComponent(words.join(" "));
}

function wikipediaInLanguage(href: string, lang: string | undefined): string {
  if (!lang) return href;
  return href.replace(/^(https?:\/\/)(?:[a-z-]+\.)?wikipedia\.org/i, `$1${lang}.wikipedia.org`);
}
export function resolveSourceParagraphHref(
  ref: EventSourceRef,
  defaultLang = "en",
): string | null {
  const spoken = ref.language?.slice(0, 2).toLowerCase();
  const resolved = resolveSourceRefHref(ref, spoken || defaultLang);
  if (!resolved) return null;
  const base = spoken === "fr" || spoken === "en" ? wikipediaInLanguage(resolved, spoken) : resolved;
  const section = ref.section_title?.trim();
  const articleSection = section && !isInternalSection(section) ? wikipediaSectionSlug(section) : "";
  const fragment = textFragmentFromQuote(ref.snippet ?? ref.quote ?? "");
  if (!articleSection && !fragment) return base.split("#")[0];
  const withoutHash = base.split("#")[0];
  if (articleSection && fragment) return `${withoutHash}#${articleSection}:~:text=${fragment}`;
  if (fragment) return `${withoutHash}#:~:text=${fragment}`;
  return `${withoutHash}#${articleSection}`;
}

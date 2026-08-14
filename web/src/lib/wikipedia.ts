// web/src/lib/wikipedia.ts
export interface WikipediaRestSummary {
  title?: string;
  extract?: string;
  thumbnailSource?: string;
  url?: string;
}

const summaryCache = new Map<string, WikipediaRestSummary | null>();

export function wikipediaArticleUrl(lang: string, title: string): string {
  return `https://${lang}.wikipedia.org/wiki/${encodeURIComponent(title.replace(/ /g, "_"))}`;
}

/** Cached Wikipedia REST summary lookup by language + title. Requires a thumbnail. */
export async function fetchWikipediaPageSummary(
  lang: string,
  title: string,
): Promise<WikipediaRestSummary | null> {
  const normalizedLang = lang.trim().toLowerCase() || "en";
  const normalizedTitle = title.trim().replace(/ /g, "_");
  if (!normalizedTitle) return null;
  const key = `${normalizedLang}:${normalizedTitle.toLowerCase()}`;
  if (summaryCache.has(key)) return summaryCache.get(key) ?? null;

  const apiUrl = `https://${normalizedLang}.wikipedia.org/api/rest_v1/page/summary/${encodeURIComponent(normalizedTitle)}`;
  try {
    const response = await fetch(apiUrl, { headers: { Accept: "application/json" } });
    if (!response.ok) {
      summaryCache.set(key, null);
      return null;
    }
    const json = (await response.json()) as {
      title?: string;
      extract?: string;
      thumbnail?: { source?: string };
      content_urls?: { desktop?: { page?: string } };
      type?: string;
    };
    if (json.type === "disambiguation") {
      summaryCache.set(key, null);
      return null;
    }
    const thumbnailSource = json.thumbnail?.source?.trim();
    if (!thumbnailSource) {
      summaryCache.set(key, null);
      return null;
    }
    const value: WikipediaRestSummary = {
      title: json.title,
      extract: json.extract?.trim(),
      thumbnailSource,
      url: json.content_urls?.desktop?.page ?? wikipediaArticleUrl(normalizedLang, title),
    };
    summaryCache.set(key, value);
    return value;
  } catch {
    summaryCache.set(key, null);
    return null;
  }
}

export async function fetchWikipediaRestSummary(
  articleUrl: string,
): Promise<WikipediaRestSummary | null> {
  const match = articleUrl.match(
    /^https?:\/\/([a-z]{2,3})\.wikipedia\.org\/wiki\/([^?#]+)/i,
  );
  if (!match) return null;
  const lang = match[1];
  const titleSegment = decodeURIComponent(match[2]);

  if (lang !== "en") {
    const english = await fetchWikipediaPageSummary("en", titleSegment);
    if (english) return english;
  }
  return fetchWikipediaPageSummary(lang, titleSegment);
}

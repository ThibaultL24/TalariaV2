// web/src/lib/wikipedia.ts
export interface WikipediaRestSummary {
  title?: string;
  extract?: string;
  thumbnailSource?: string;
  url?: string;
}

export function wikipediaArticleUrl(lang: string, title: string): string {
  return `https://${lang}.wikipedia.org/wiki/${encodeURIComponent(title.replace(/ /g, "_"))}`;
}

export async function fetchWikipediaRestSummary(
  articleUrl: string,
): Promise<WikipediaRestSummary | null> {
  const match = articleUrl.match(
    /^https?:\/\/([a-z]{2,3})\.wikipedia\.org\/wiki\/([^?#]+)/i,
  );
  if (!match) return null;
  const lang = match[1];
  const titleSegment = match[2];

  async function load(wikiLang: string): Promise<WikipediaRestSummary | null> {
    const apiUrl = `https://${wikiLang}.wikipedia.org/api/rest_v1/page/summary/${titleSegment}`;
    const response = await fetch(apiUrl, { headers: { Accept: "application/json" } });
    if (!response.ok) return null;
    const json = (await response.json()) as {
      title?: string;
      extract?: string;
      thumbnail?: { source?: string };
      content_urls?: { desktop?: { page?: string } };
    };
    const extract = json.extract?.trim();
    const thumbnailSource = json.thumbnail?.source?.trim();
    if (!extract && !thumbnailSource) return null;
    return {
      title: json.title,
      extract,
      thumbnailSource,
      url: json.content_urls?.desktop?.page ?? articleUrl,
    };
  }

  if (lang !== "en") {
    const english = await load("en");
    if (english) return english;
  }
  return load(lang);
}

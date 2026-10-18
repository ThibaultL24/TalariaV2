// web/src/features/entity/database-tabs.ts

export function providerLabel(name: string): string {
  const labels: Record<string, string> = {
    wikipedia: "Wikipedia",
    wikidata: "Wikidata",
    hal: "HAL",
    bnf: "BnF",
    gallica: "Gallica",
    persee: "Persée",
    openalex: "OpenAlex",
    open_alex: "OpenAlex",
    openlibrary: "Open Library",
    open_library: "Open Library",
    internet_archive: "Internet Archive",
    theses_fr: "theses.fr",
    europeana: "Europeana",
    commons: "Wikimedia Commons",
    wikimedia_commons: "Wikimedia Commons",
    wikisource: "Wikisource",
  };
  return labels[name] ?? name.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}

export function databaseTabs(providers: { name: string; count: number }[]) {
  const names = [
    "wikipedia", "wikidata", "open_alex", "persee", "hal", "gallica", "bnf",
    "theses_fr", "open_library", "internet_archive", "europeana", "wikisource",
    "wikimedia_commons", "crossref", "open_edition", "sudoc", "france_archives",
    "deutsche_biographie", "pop_merimee",
  ];
  return [...new Set([...names, ...providers.map((item) => item.name)])].map((name) => ({
    name,
    count: providers.find((item) => item.name === name)?.count ?? 0,
  }));
}

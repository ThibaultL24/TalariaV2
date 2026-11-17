// web/src/features/istoria/chapter-suggestions.tsx

export interface ChapterSuggestion {
  slug: string;
  title_fr: string;
  title_en: string;
  event_count: number;
  standing: string;
  basis_fr: string;
  basis_en: string;
}

export function ChapterSuggestions({
  suggestions,
  locale,
}: {
  suggestions: ChapterSuggestion[];
  locale: "fr" | "en";
}) {
  if (suggestions.length === 0) return null;
  const fr = locale === "fr";
  return (
    <section aria-label={fr ? "Suggestions de chapitres" : "Chapter suggestions"}>
      <h2>{fr ? "Suggestions" : "Suggestions"}</h2>
      <p>
        {fr
          ? "Ces regroupements suivent les faits déjà documentés. Ce ne sont pas des chapitres publiés."
          : "These groupings follow documented facts. They are not published chapters."}
      </p>
      <ul>
        {suggestions.map((item) => (
          <li key={item.slug}>
            <span>{fr ? item.title_fr : item.title_en}</span>
            {` · ${item.standing}`}
            <p>{fr ? item.basis_fr : item.basis_en}</p>
          </li>
        ))}
      </ul>
    </section>
  );
}

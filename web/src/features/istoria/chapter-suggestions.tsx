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
  periods = [],
  locale,
}: {
  suggestions: ChapterSuggestion[];
  periods?: ChapterSuggestion[];
  locale: "fr" | "en";
}) {
  if (suggestions.length === 0 && periods.length === 0) return null;
  const fr = locale === "fr";
  return (
    <section aria-label={fr ? "Suggestions de chapitres" : "Chapter suggestions"}>
      <p>
        {fr
          ? "Ces regroupements suivent les faits déjà admis. Ce ne sont pas des chapitres publiés, ni un récit confirmé."
          : "These groupings follow admitted facts. They are not published chapters, and they are not a confirmed narrative."}
      </p>
      {periods.length > 0 && (
        <>
          <h2>{fr ? "Périodes de la vie" : "Life periods"}</h2>
          <ul>
            {periods.map((item) => (
              <li key={item.slug}>
                <span>{fr ? item.title_fr : item.title_en}</span>
                {` · ${item.event_count}`}
                <p>{fr ? item.basis_fr : item.basis_en}</p>
              </li>
            ))}
          </ul>
        </>
      )}
      {suggestions.length > 0 && (
        <>
          <h2>{fr ? "Par type de fait" : "By kind of fact"}</h2>
          <ul>
            {suggestions.map((item) => (
              <li key={item.slug}>
                <span>{fr ? item.title_fr : item.title_en}</span>
                {` · ${item.event_count}`}
                <p>{fr ? item.basis_fr : item.basis_en}</p>
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}

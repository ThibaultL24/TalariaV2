// web/src/features/istoria/chapter-suggestions.tsx

export interface ChapterSuggestion {
  slug: string;
  title_fr: string;
  title_en: string;
  event_count: number;
  standing: string;
  basis_fr: string;
  basis_en: string;
  members?: string[];
}

function ChapterList({
  title,
  items,
  fr,
}: {
  title: string;
  items: ChapterSuggestion[];
  fr: boolean;
}) {
  if (items.length === 0) return null;
  return (
    <>
      <h2>{title}</h2>
      <ul>
        {items.map((item) => (
          <li key={item.slug}>
            <span>{fr ? item.title_fr : item.title_en}</span>
            {` · ${item.event_count}`}
            <p>{fr ? item.basis_fr : item.basis_en}</p>
            {item.members && item.members.length > 0 && (
              <p>{item.members.slice(0, 6).join(" · ")}</p>
            )}
          </li>
        ))}
      </ul>
    </>
  );
}

export function ChapterSuggestions({
  suggestions,
  periods = [],
  sections = [],
  places = [],
  locale,
}: {
  suggestions: ChapterSuggestion[];
  periods?: ChapterSuggestion[];
  sections?: ChapterSuggestion[];
  places?: ChapterSuggestion[];
  locale: "fr" | "en";
}) {
  if (suggestions.length + periods.length + sections.length + places.length === 0) return null;
  const fr = locale === "fr";
  return (
    <section aria-label={fr ? "Suggestions de chapitres" : "Chapter suggestions"}>
      <p>
        {fr
          ? "Ces regroupements suivent les faits déjà admis. Ce ne sont pas des chapitres publiés, ni un récit confirmé. Une suite de lieux n'est pas un itinéraire."
          : "These groupings follow admitted facts. They are not published chapters, and they are not a confirmed narrative. A sequence of places is not an itinerary."}
      </p>
      <ChapterList title={fr ? "Périodes de la vie" : "Life periods"} items={periods} fr={fr} />
      <ChapterList title={fr ? "Sections de l'article" : "Article sections"} items={sections} fr={fr} />
      <ChapterList title={fr ? "Lieux qui reviennent" : "Repeated places"} items={places} fr={fr} />
      <ChapterList title={fr ? "Par type de fait" : "By kind of fact"} items={suggestions} fr={fr} />
    </section>
  );
}

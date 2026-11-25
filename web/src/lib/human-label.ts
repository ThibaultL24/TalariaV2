// web/src/lib/human-label.ts
const PREDICATES: Record<string, string> = {
  resided_in: "résidence",
  born_in: "naissance",
  died_in: "mort",
  studied_at: "études",
  position_held: "charge",
  spouse: "union",
};

/** A reader sees a word, not a Wikidata identifier. */
export function readableLabel(value: string): string {
  const words = value
    .replace(/\bQ\d+\b/g, " ")
    .replace(/\|/g, " ")
    .split(/\s+/)
    .filter(Boolean)
    .map((word) => PREDICATES[word] ?? word);
  return words.join(" ");
}

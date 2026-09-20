// web/src/lib/demo-roster.ts
/** Curated demo pack — historical + modern figures for the public showcase. */

export type DemoEra = "historical" | "modern";

export interface DemoRosterEntry {
  qid: string;
  labelEn: string;
  labelFr: string;
  era: DemoEra;
  /** Wikipedia title hint for ingest / search */
  wikiTitle: string;
  intuitionStar?: boolean;
}

export const DEMO_ROSTER: DemoRosterEntry[] = [
  {
    qid: "Q517",
    labelEn: "Napoleon",
    labelFr: "Napoléon",
    era: "historical",
    wikiTitle: "Napoleon",
  },
  {
    qid: "Q687",
    labelEn: "Molière",
    labelFr: "Molière",
    era: "historical",
    wikiTitle: "Molière",
  },
  {
    qid: "Q7186",
    labelEn: "Marie Curie",
    labelFr: "Marie Curie",
    era: "historical",
    wikiTitle: "Marie Curie",
  },
  {
    qid: "Q535",
    labelEn: "Victor Hugo",
    labelFr: "Victor Hugo",
    era: "historical",
    wikiTitle: "Victor Hugo",
  },
  {
    qid: "Q7226",
    labelEn: "Joan of Arc",
    labelFr: "Jeanne d'Arc",
    era: "historical",
    wikiTitle: "Joan of Arc",
  },
  {
    qid: "Q7742",
    labelEn: "Louis XIV",
    labelFr: "Louis XIV",
    era: "historical",
    wikiTitle: "Louis XIV",
  },
  {
    qid: "Q3052772",
    labelEn: "Emmanuel Macron",
    labelFr: "Emmanuel Macron",
    era: "modern",
    wikiTitle: "Emmanuel Macron",
  },
  {
    qid: "Q22686",
    labelEn: "Donald Trump",
    labelFr: "Donald Trump",
    era: "modern",
    wikiTitle: "Donald Trump",
  },
  {
    qid: "Q2042",
    labelEn: "Charles de Gaulle",
    labelFr: "Charles de Gaulle",
    era: "modern",
    wikiTitle: "Charles de Gaulle",
  },
  {
    qid: "Q76",
    labelEn: "Barack Obama",
    labelFr: "Barack Obama",
    era: "modern",
    wikiTitle: "Barack Obama",
  },
];

export const DEMO_ROSTER_QIDS = DEMO_ROSTER.map((entry) => entry.qid);

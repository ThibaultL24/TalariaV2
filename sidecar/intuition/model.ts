// sidecar/intuition/model.ts
// Off-chain debate graph via MetaSudo Alpha packages. No RPC.

import { buildAtomData } from "@0xintuition/classifications";
import {
  calculateAtomId,
  calculatePredicateId,
  calculateTripleId,
  createPredicateAtomData,
} from "@0xintuition/ids";
import { HAS_CATEGORY_ID, HAS_TAG_ID } from "@0xintuition/predicates";

export interface DebateText {
  text: string;
}

export interface AboutEvent {
  canonical_event_id: string;
  title: string;
  event_type: string;
  time_surface: string;
}

export interface DebateFact {
  version: string;
  debate_id: string;
  kind: string;
  question: DebateText;
  proposition: DebateText;
  about_event?: AboutEvent | null;
}

export interface ModeledAtom {
  role: string;
  classification: string;
  name: string;
  sameAs?: string;
  startDate?: string;
  atomData: string;
  atomId: `0x${string}`;
}

export interface ModeledTriple {
  role: string;
  subjectId: `0x${string}`;
  predicateId: `0x${string}`;
  objectId: `0x${string}`;
}

export interface DebateGraph {
  version: string;
  debate_id: string;
  kind: string;
  category: string;
  atoms: ModeledAtom[];
  triples: ModeledTriple[];
  voteTripleId: `0x${string}`;
  eventAtom?: ModeledAtom;
}

const HAS_PROPOSITION_DATA = createPredicateAtomData(
  "hasProposition",
  "The question has this proposition as a vote target",
);
const ABOUT_DATA = createPredicateAtomData(
  "about",
  "The proposition is about this classified Talaria event pointer",
);
const HAS_PROPOSITION_ID = calculatePredicateId(
  "hasProposition",
  "The question has this proposition as a vote target",
);
const ABOUT_ID = calculatePredicateId(
  "about",
  "The proposition is about this classified Talaria event pointer",
);

export function categoryTerm(fact: DebateFact): string {
  const eventType = fact.about_event?.event_type?.trim() ?? "";
  if (eventType.length > 0) return eventType;
  const kind = fact.kind.trim();
  if (kind.length === 0 || kind === "place_conflict") return "uncategorized";
  return kind;
}

export function startDateField(timeSurface: string): string | undefined {
  const s = timeSurface.trim();
  if (/^\d{4}-\d{2}-\d{2}$/.test(s)) return s;
  return undefined;
}

export function eventAtomName(canonicalEventId: string): string {
  return `canonical-event:${canonicalEventId}`;
}

export function eventSameAs(canonicalEventId: string): string {
  return `talaria://canonical-event/${canonicalEventId}`;
}

function definedTerm(role: string, name: string): ModeledAtom {
  const atomData = buildAtomData("defined-term", { name });
  return {
    role,
    classification: "defined-term",
    name,
    atomData,
    atomId: calculateAtomId(atomData),
  };
}

function eventAtom(ev: AboutEvent): ModeledAtom {
  const name = eventAtomName(ev.canonical_event_id);
  const sameAs = eventSameAs(ev.canonical_event_id);
  const startDate = startDateField(ev.time_surface);
  const values: Record<string, unknown> = {
    name,
    sameAs: [sameAs],
  };
  if (startDate) values.startDate = startDate;
  const atomData = buildAtomData("event", values);
  return {
    role: "event",
    classification: "event",
    name,
    sameAs,
    startDate,
    atomData,
    atomId: calculateAtomId(atomData),
  };
}

export function modelDebate(fact: DebateFact): DebateGraph {
  const category = categoryTerm(fact);
  const question = definedTerm("question", fact.question.text);
  const proposition = definedTerm("proposition", fact.proposition.text);
  const categoryAtom = definedTerm("category", category);
  const atoms: ModeledAtom[] = [question, proposition, categoryAtom];
  const triples: ModeledTriple[] = [
    {
      role: "question_has_proposition",
      subjectId: question.atomId,
      predicateId: HAS_PROPOSITION_ID,
      objectId: proposition.atomId,
    },
    {
      role: "question_has_category",
      subjectId: question.atomId,
      predicateId: HAS_CATEGORY_ID,
      objectId: categoryAtom.atomId,
    },
  ];

  let event: ModeledAtom | undefined;
  if (fact.about_event?.canonical_event_id) {
    event = eventAtom(fact.about_event);
    atoms.push(event);
    triples.push({
      role: "proposition_about_event",
      subjectId: proposition.atomId,
      predicateId: ABOUT_ID,
      objectId: event.atomId,
    });
  }

  if (
    fact.about_event?.event_type?.trim() &&
    fact.kind !== "place_conflict" &&
    fact.kind.trim().length > 0
  ) {
    const tag = definedTerm("kind_tag", fact.kind.trim());
    atoms.push(tag);
    triples.push({
      role: "question_has_tag",
      subjectId: question.atomId,
      predicateId: HAS_TAG_ID,
      objectId: tag.atomId,
    });
  }

  const vote = triples.find((t) => t.role === "question_has_proposition");
  if (!vote) throw new Error("missing vote-target triple");

  return {
    version: fact.version,
    debate_id: fact.debate_id,
    kind: fact.kind,
    category,
    atoms,
    triples,
    voteTripleId: calculateTripleId(vote.subjectId, vote.predicateId, vote.objectId),
    eventAtom: event,
  };
}

export const PREDICATE_ATOM_DATA = {
  hasProposition: HAS_PROPOSITION_DATA,
  about: ABOUT_DATA,
};

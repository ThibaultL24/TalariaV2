// sidecar/intuition/model.ts
// Pure DebateFact → logical graph. No RPC. No definitive on-chain term ids.

import { buildAtomData } from "@0xintuition/classifications";
import type { PredicateKey } from "./predicates.ts";
import type { DebateFact } from "./schemas.ts";

export type { DebateFact } from "./schemas.ts";

export interface PinPayload {
  name: string;
  description: string;
  url: string;
}

export interface LogicalAtom {
  role: string;
  classification: string;
  name: string;
  sameAs?: string;
  startDate?: string;
  /** Metadata for pinThing — on-chain atom data will be the returned ipfs:// URI. */
  pin: PinPayload;
}

export interface LogicalTriple {
  role: string;
  subjectRole: string;
  predicateKey: PredicateKey;
  objectRole: string;
}

export interface DebateGraph {
  version: string;
  debate_id: string;
  kind: string;
  category: string;
  atoms: LogicalAtom[];
  triples: LogicalTriple[];
  voteTripleRole: "question_has_proposition";
}

export function categoryTerm(fact: DebateFact): string {
  const eventType = fact.about_event?.event_type?.trim() ?? "";
  if (eventType.length > 0) return eventType;
  const kind = fact.kind.trim();
  if (kind.length === 0 || kind === "place_conflict") return "uncategorized";
  return kind;
}

/** Full calendar day only — never promote year/month surfaces. */
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

function definedTerm(role: string, name: string): LogicalAtom {
  return {
    role,
    classification: "defined-term",
    name,
    pin: {
      name,
      description: "defined-term",
      url: "",
    },
  };
}

function eventAtom(ev: NonNullable<DebateFact["about_event"]>): LogicalAtom {
  const name = eventAtomName(ev.canonical_event_id);
  const sameAs = eventSameAs(ev.canonical_event_id);
  const startDate = startDateField(ev.time_surface);
  // Keep classification helper for offline fixtures; pin carries identity.
  void buildAtomData("event", {
    name,
    sameAs: [sameAs],
    ...(startDate ? { startDate } : {}),
  });
  return {
    role: "event",
    classification: "event",
    name,
    sameAs,
    startDate,
    pin: {
      name,
      description: "event",
      url: sameAs,
    },
  };
}

export function modelDebate(fact: DebateFact): DebateGraph {
  const category = categoryTerm(fact);
  const question = definedTerm("question", fact.question.text);
  const proposition = definedTerm("proposition", fact.proposition.text);
  const categoryAtom = definedTerm("category", category);
  const atoms: LogicalAtom[] = [question, proposition, categoryAtom];
  const triples: LogicalTriple[] = [
    {
      role: "question_has_proposition",
      subjectRole: "question",
      predicateKey: "hasProposition",
      objectRole: "proposition",
    },
    {
      role: "question_has_category",
      subjectRole: "question",
      predicateKey: "hasCategory",
      objectRole: "category",
    },
  ];

  if (fact.about_event?.canonical_event_id) {
    atoms.push(eventAtom(fact.about_event));
    triples.push({
      role: "proposition_about_event",
      subjectRole: "proposition",
      predicateKey: "about",
      objectRole: "event",
    });
  }

  if (
    fact.about_event?.event_type?.trim() &&
    fact.kind !== "place_conflict" &&
    fact.kind.trim().length > 0
  ) {
    atoms.push(definedTerm("kind_tag", fact.kind.trim()));
    triples.push({
      role: "question_has_tag",
      subjectRole: "question",
      predicateKey: "hasTag",
      objectRole: "kind_tag",
    });
  }

  return {
    version: fact.version,
    debate_id: fact.debate_id,
    kind: fact.kind,
    category,
    atoms,
    triples,
    voteTripleRole: "question_has_proposition",
  };
}

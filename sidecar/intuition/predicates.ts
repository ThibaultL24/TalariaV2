// sidecar/intuition/predicates.ts
// Official Intuition predicates + Talaria-specific predicates (deterministic ids).

import {
  calculatePredicateId,
  createPredicateAtomData,
} from "@0xintuition/ids";
import {
  HAS_CATEGORY_ID,
  HAS_TAG_ID,
  PREDICATE_DEFS,
} from "@0xintuition/predicates";

/** Predicates not yet in @0xintuition/predicates — identity must stay deterministic. */
const TALARIA_PREDICATE_DEFS = {
  hasProposition: {
    name: "hasProposition",
    description: "The question has this proposition as a vote target",
  },
  about: {
    name: "about",
    description: "The proposition is about this classified Talaria event pointer",
  },
} as const;

export type PredicateKey =
  | "hasProposition"
  | "about"
  | "hasCategory"
  | "hasTag";

export function predicateAtomData(key: PredicateKey): string {
  switch (key) {
    case "hasProposition":
      return createPredicateAtomData(
        TALARIA_PREDICATE_DEFS.hasProposition.name,
        TALARIA_PREDICATE_DEFS.hasProposition.description,
      );
    case "about":
      return createPredicateAtomData(
        TALARIA_PREDICATE_DEFS.about.name,
        TALARIA_PREDICATE_DEFS.about.description,
      );
    case "hasCategory":
      return createPredicateAtomData(
        PREDICATE_DEFS.hasCategory.name,
        PREDICATE_DEFS.hasCategory.description,
      );
    case "hasTag":
      return createPredicateAtomData(
        PREDICATE_DEFS.hasTag.name,
        PREDICATE_DEFS.hasTag.description,
      );
  }
}

/** Off-chain / registry id for wiring logical triples (not a post-pin atom id). */
export function predicateRegistryId(key: PredicateKey): `0x${string}` {
  switch (key) {
    case "hasProposition":
      return calculatePredicateId(
        TALARIA_PREDICATE_DEFS.hasProposition.name,
        TALARIA_PREDICATE_DEFS.hasProposition.description,
      );
    case "about":
      return calculatePredicateId(
        TALARIA_PREDICATE_DEFS.about.name,
        TALARIA_PREDICATE_DEFS.about.description,
      );
    case "hasCategory":
      return HAS_CATEGORY_ID;
    case "hasTag":
      return HAS_TAG_ID;
  }
}

export const TALARIA_PREDICATES = TALARIA_PREDICATE_DEFS;

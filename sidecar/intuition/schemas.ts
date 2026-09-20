// sidecar/intuition/schemas.ts
// Boundary validation for DebateFact (v2 snake_case + v3 camelCase envelope).

import { z } from "zod";

export const SCHEMA_VERSION_V2 = "talaria.intuition_canon.v2";
export const SCHEMA_VERSION_V3 = "talaria.intuition_canon.v3";

const DebateTextSchema = z.object({
  text: z.string().trim().min(1, "text required"),
});

const AboutEventV2Schema = z.object({
  canonical_event_id: z.string().trim().min(1),
  title: z.string(),
  event_type: z.string(),
  time_surface: z.string(),
});

const DebateFactV2Schema = z.object({
  version: z.literal(SCHEMA_VERSION_V2),
  debate_id: z.string().trim().min(1),
  kind: z.string().trim().min(1),
  question: DebateTextSchema,
  proposition: DebateTextSchema,
  about_event: AboutEventV2Schema.nullish(),
});

const TypedTimeV3Schema = z.object({
  kind: z.string().optional(),
  start: z.string().trim().min(1),
  precision: z.string().optional(),
  calendar: z.string().optional(),
  end: z.string().optional(),
});

const AboutEventV3Schema = z.object({
  id: z.string().trim().min(1),
  type: z.string().default(""),
  title: z.string().optional(),
  time: TypedTimeV3Schema.optional(),
  time_surface: z.string().optional(),
});

const FactBodyV3Schema = z.object({
  kind: z.string().trim().min(1),
  debate_id: z.string().trim().min(1).optional(),
  question: DebateTextSchema,
  proposition: DebateTextSchema,
  aboutEvent: AboutEventV3Schema.nullish(),
});

const EnvelopeV3Schema = z.object({
  version: z.literal(SCHEMA_VERSION_V3),
  publicationId: z.string().uuid().optional(),
  network: z.enum(["testnet", "mainnet"]).default("testnet"),
  fact: FactBodyV3Schema,
});

export type DebateFact = {
  version: string;
  debate_id: string;
  kind: string;
  question: { text: string };
  proposition: { text: string };
  about_event?: {
    canonical_event_id: string;
    title: string;
    event_type: string;
    time_surface: string;
  } | null;
};

function timeSurfaceFromV3(ev: z.infer<typeof AboutEventV3Schema>): string {
  if (ev.time_surface?.trim()) return ev.time_surface.trim();
  const start = ev.time?.start?.trim() ?? "";
  // Preserve typed precision: never invent -01-01 for year-only starts.
  return start;
}

function fromV3(envelope: z.infer<typeof EnvelopeV3Schema>): DebateFact {
  const f = envelope.fact;
  const about = f.aboutEvent
    ? {
        canonical_event_id: f.aboutEvent.id,
        title: f.aboutEvent.title ?? "",
        event_type: f.aboutEvent.type,
        time_surface: timeSurfaceFromV3(f.aboutEvent),
      }
    : null;
  const debate_id =
    f.debate_id?.trim() ||
    `talaria:debate:${envelope.publicationId ?? "anon"}:${f.kind}`;
  return {
    version: SCHEMA_VERSION_V3,
    debate_id,
    kind: f.kind,
    question: { text: f.question.text.trim() },
    proposition: { text: f.proposition.text.trim() },
    about_event: about,
  };
}

function fromV2(raw: z.infer<typeof DebateFactV2Schema>): DebateFact {
  return {
    version: raw.version,
    debate_id: raw.debate_id,
    kind: raw.kind,
    question: { text: raw.question.text.trim() },
    proposition: { text: raw.proposition.text.trim() },
    about_event: raw.about_event
      ? {
          canonical_event_id: raw.about_event.canonical_event_id,
          title: raw.about_event.title,
          event_type: raw.about_event.event_type,
          time_surface: raw.about_event.time_surface,
        }
      : null,
  };
}

/** Accept v2 DebateFact or v3 publish envelope; always return internal DebateFact. */
export function parseDebateFactInput(input: unknown): DebateFact {
  if (input && typeof input === "object" && "version" in input) {
    const version = (input as { version?: unknown }).version;
    if (version === SCHEMA_VERSION_V3) {
      return fromV3(EnvelopeV3Schema.parse(input));
    }
    if (version === SCHEMA_VERSION_V2) {
      return fromV2(DebateFactV2Schema.parse(input));
    }
  }
  // Bare v2-shaped object without version → treat as v2 for CLI compat.
  const withVersion = {
    version: SCHEMA_VERSION_V2,
    ...(input as Record<string, unknown>),
  };
  return fromV2(DebateFactV2Schema.parse(withVersion));
}

export const PublishErrorSchema = z.object({
  status: z.literal("failed"),
  stage: z.enum([
    "pin",
    "resolve",
    "simulate",
    "broadcast",
    "confirm",
    "verify",
    "persist",
  ]),
  code: z.string(),
  message: z.string(),
  retryable: z.boolean(),
});

export type PublishError = z.infer<typeof PublishErrorSchema>;

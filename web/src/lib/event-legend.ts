// web/src/lib/event-legend.ts
import type { AppLocale } from "@/lib/i18n";
import type { GeoJsonFeatureCollection } from "@/lib/api";

export type LegendKey =
  | "life"
  | "conflict"
  | "travel"
  | "office"
  | "work"
  | "anecdote"
  | "legacy";

export const LEGEND_COLORS: Record<LegendKey, string> = {
  life: "#7dd3fc",
  conflict: "#f87171",
  travel: "#34d399",
  office: "#c4b5fd",
  work: "#fb923c",
  anecdote: "#e8b84a",
  legacy: "#94a3b8",
};

const LEGEND_LABELS: Record<AppLocale, Record<LegendKey, string>> = {
  en: {
    life: "Birth, death, family",
    conflict: "Battles & conflict",
    travel: "Travel & residence",
    office: "Office & diplomacy",
    work: "Study, work, creation",
    anecdote: "Anecdotes & facts",
    legacy: "Memorials",
  },
  fr: {
    life: "Naissance, mort, famille",
    conflict: "Batailles et conflits",
    travel: "Voyages et résidences",
    office: "Charges et diplomatie",
    work: "Études, œuvre, création",
    anecdote: "Anecdotes et faits",
    legacy: "Mémoire",
  },
};

const TYPE_TO_LEGEND: Record<string, LegendKey> = {
  birth: "life",
  death: "life",
  marriage: "life",
  divorce: "life",
  battle: "conflict",
  siege: "conflict",
  imprisonment: "conflict",
  travel: "travel",
  relocation: "travel",
  residence: "travel",
  exile: "travel",
  meeting: "travel",
  office: "office",
  diplomatic: "office",
  employment: "office",
  education: "work",
  publication: "work",
  creation: "work",
  discovery: "work",
  award: "work",
  speech: "work",
  anecdote: "anecdote",
  historical_fact: "anecdote",
  life_event: "anecdote",
  statue: "legacy",
  museum: "legacy",
  street_naming: "legacy",
  memorial: "legacy",
};

export const LEGEND_ORDER: LegendKey[] = [
  "life",
  "conflict",
  "travel",
  "office",
  "work",
  "anecdote",
  "legacy",
];

export function legendKeyForEventType(eventType: string): LegendKey {
  return TYPE_TO_LEGEND[eventType] ?? "anecdote";
}

export function legendLabel(key: LegendKey, locale: AppLocale): string {
  return LEGEND_LABELS[locale][key];
}

export function attachLegendKeys(
  collection: GeoJsonFeatureCollection,
): GeoJsonFeatureCollection {
  return {
    type: "FeatureCollection",
    features: collection.features.map((feature) => {
      const eventType = String(feature.properties.event_type ?? "");
      return {
        ...feature,
        properties: {
          ...feature.properties,
          legend_key: legendKeyForEventType(eventType),
        },
      };
    }),
  };
}

export function mapLibreLegendColorExpr(): unknown[] {
  return [
    "match",
    ["coalesce", ["get", "legend_key"], "anecdote"],
    "life",
    LEGEND_COLORS.life,
    "conflict",
    LEGEND_COLORS.conflict,
    "travel",
    LEGEND_COLORS.travel,
    "office",
    LEGEND_COLORS.office,
    "work",
    LEGEND_COLORS.work,
    "anecdote",
    LEGEND_COLORS.anecdote,
    "legacy",
    LEGEND_COLORS.legacy,
    LEGEND_COLORS.anecdote,
  ];
}

// web/src/lib/agora-taxonomy.ts
import type { AppLocale } from "@/stores/locale-store";

const DEBATE_TYPE_LABELS: Record<AppLocale, Record<string, string>> = {
  en: {
    birth_date: "Birth date",
    nationality_origins: "Origins & nationality",
    hero_villain: "Hero / villain framing",
    revisionism: "Revisionism",
    controversy: "Controversy",
    interpretation: "Interpretation",
    attribution: "Attribution",
  },
  fr: {
    birth_date: "Date de naissance",
    nationality_origins: "Origines et nationalité",
    hero_villain: "Héros / vilain",
    revisionism: "Révisionnisme",
    controversy: "Controverse",
    interpretation: "Interprétation",
    attribution: "Attribution",
  },
};

const EVIDENCE_LAYER_LABELS: Record<AppLocale, Record<string, string>> = {
  en: {
    historiography: "Historiography",
    academic_abstract: "Academic abstract",
    catalog_metadata: "Catalog metadata",
    thesis: "Thesis",
  },
  fr: {
    historiography: "Historiographie",
    academic_abstract: "Résumé académique",
    catalog_metadata: "Métadonnées de catalogue",
    thesis: "Thèse",
  },
};

export function debateTypeLabel(
  value: string | null | undefined,
  locale: AppLocale = "en",
): string | null {
  if (!value) return null;
  const key = value.trim().toLowerCase();
  return DEBATE_TYPE_LABELS[locale][key] ?? value.replace(/_/g, " ");
}

export function evidenceLayerLabel(
  value: string | null | undefined,
  locale: AppLocale = "en",
): string | null {
  if (!value) return null;
  const key = value.trim().toLowerCase();
  return EVIDENCE_LAYER_LABELS[locale][key] ?? value.replace(/_/g, " ");
}

export function groupClaimsByDebateType<T extends { debate_type?: string | null }>(
  claims: T[],
  locale: AppLocale = "en",
  otherLabel = "Other debates",
): Array<{ key: string; label: string; claims: T[] }> {
  const buckets = new Map<string, T[]>();
  for (const claim of claims) {
    const key = (claim.debate_type ?? "other").trim().toLowerCase() || "other";
    const list = buckets.get(key) ?? [];
    list.push(claim);
    buckets.set(key, list);
  }
  return [...buckets.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([key, group]) => ({
      key,
      label: key === "other" ? otherLabel : (debateTypeLabel(key, locale) ?? otherLabel),
      claims: group,
    }));
}

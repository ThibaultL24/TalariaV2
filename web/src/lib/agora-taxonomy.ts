// web/src/lib/agora-taxonomy.ts
const DEBATE_TYPE_LABELS: Record<string, string> = {
  birth_date: "Birth date",
  nationality_origins: "Origins & nationality",
  hero_villain: "Hero / villain framing",
  revisionism: "Revisionism",
  controversy: "Controversy",
  interpretation: "Interpretation",
  attribution: "Attribution",
};

const EVIDENCE_LAYER_LABELS: Record<string, string> = {
  historiography: "Historiography",
  academic_abstract: "Academic abstract",
  catalog_metadata: "Catalog metadata",
  thesis: "Thesis",
};

export function debateTypeLabel(value: string | null | undefined): string | null {
  if (!value) return null;
  const key = value.trim().toLowerCase();
  return DEBATE_TYPE_LABELS[key] ?? value.replace(/_/g, " ");
}

export function evidenceLayerLabel(value: string | null | undefined): string | null {
  if (!value) return null;
  const key = value.trim().toLowerCase();
  return EVIDENCE_LAYER_LABELS[key] ?? value.replace(/_/g, " ");
}

export function groupClaimsByDebateType<T extends { debate_type?: string | null }>(
  claims: T[],
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
      label: debateTypeLabel(key) ?? "Other debates",
      claims: group,
    }));
}

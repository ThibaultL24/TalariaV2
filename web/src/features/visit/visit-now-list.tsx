// web/src/features/visit/visit-now-list.tsx
import type { VisitOpportunity } from "@/lib/entity-views";
import { useI18n } from "@/lib/i18n";

function formatWindowDate(iso?: string | null): string {
  if (!iso) return "";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
}

export function VisitNowList({
  items,
  loading,
  onSelect,
  emptyLabel,
}: {
  items: VisitOpportunity[];
  loading?: boolean;
  onSelect: (id: string) => void;
  emptyLabel: string;
}) {
  const { t } = useI18n();
  if (loading && !items.length) {
    return <p className="v3-note">{t.loading}</p>;
  }
  if (!items.length) {
    return <p className="v3-note">{emptyLabel}</p>;
  }
  return (
    <ul className="visit-heritage-list visit-now-list">
      {items.map((item) => (
        <li key={item.id}>
          <button type="button" onClick={() => onSelect(item.id)}>
            <span className="visit-heritage-list__meta">
              {item.kind.replaceAll("_", " ")} · {item.source_kind}
              {item.ends_at ? ` · ${t.visitNowEnds} ${formatWindowDate(item.ends_at)}` : null}
            </span>
            <span className="visit-heritage-list__title">{item.title}</span>
            {item.venue_label ? (
              <span className="visit-heritage-list__place">{item.venue_label}</span>
            ) : null}
            {item.canonical_url ? (
              <a
                className="visit-now-list__link"
                href={item.canonical_url}
                target="_blank"
                rel="noreferrer"
                onClick={(e) => e.stopPropagation()}
              >
                {t.visitNowOpenSource}
              </a>
            ) : null}
          </button>
        </li>
      ))}
    </ul>
  );
}

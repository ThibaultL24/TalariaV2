// web/src/components/map/map-legend.tsx
import { LEGEND_COLORS, LEGEND_ORDER, legendLabel, type LegendKey } from "@/lib/event-legend";
import { useI18n } from "@/lib/i18n";

interface MapLegendProps {
  presentKeys: LegendKey[];
}

export function MapLegend({ presentKeys }: MapLegendProps) {
  const { locale, t } = useI18n();
  const keys = LEGEND_ORDER.filter((key) => presentKeys.includes(key));
  if (keys.length === 0) return null;

  return (
    <aside
      className="surface-nav pointer-events-auto absolute right-3 bottom-[9.5rem] z-10 w-[min(100%,16rem)] rounded-xl px-3 py-2.5 sm:bottom-[8.75rem]"
      aria-label={t.legendTitle}
    >
      <p className="mb-2 text-[10px] font-semibold uppercase tracking-wide text-(--color-text-muted)">
        {t.legendTitle}
      </p>
      <ul className="space-y-1.5">
        {keys.map((key) => (
          <li key={key} className="flex items-center gap-2 text-[11px] text-(--color-text-secondary)">
            <span
              className="inline-block h-2.5 w-2.5 shrink-0 rounded-full"
              style={{ background: LEGEND_COLORS[key] }}
              aria-hidden
            />
            <span>{legendLabel(key, locale)}</span>
          </li>
        ))}
      </ul>
    </aside>
  );
}

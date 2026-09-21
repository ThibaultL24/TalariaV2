// web/src/components/map/map-legend.tsx
import {
  LEGEND_COLORS,
  LEGEND_ORDER,
  legendKeyIsActive,
  legendLabel,
  type LegendKey,
} from "@/lib/event-legend";
import { useI18n } from "@/lib/i18n";

interface MapLegendProps {
  presentKeys: LegendKey[];
  selectedKeys: LegendKey[];
  onToggleKey: (key: LegendKey) => void;
  onClear?: () => void;
}

export function MapLegend({
  presentKeys,
  selectedKeys,
  onToggleKey,
  onClear,
}: MapLegendProps) {
  const { locale, t } = useI18n();
  const keys = LEGEND_ORDER.filter((key) => presentKeys.includes(key));
  if (keys.length === 0) return null;

  const filtered = selectedKeys.length > 0;

  return (
    <aside
      className="explorer-legend surface-nav shrink-0 rounded-xl px-3 py-2"
      aria-label={t.legendTitle}
    >
      <div className="mb-2 flex items-center justify-between gap-2">
        <p className="text-[10px] font-semibold uppercase tracking-wide text-(--color-text-muted)">
          {t.legendTitle}
        </p>
        {filtered && onClear ? (
          <button
            type="button"
            onClick={onClear}
            className="text-[10px] text-(--color-accent-strong) hover:underline"
          >
            {t.showAll}
          </button>
        ) : (
          <span className="text-[9px] text-(--color-text-muted)">{t.legendClickHint}</span>
        )}
      </div>
      <ul className="space-y-0.5">
        {keys.map((key) => {
          const active = legendKeyIsActive(key, selectedKeys);
          const dimmed = filtered && !active;
          return (
            <li key={key}>
              <button
                type="button"
                onClick={() => onToggleKey(key)}
                aria-pressed={filtered ? active : false}
                className={`flex w-full items-center gap-2 rounded-md px-1 py-1 text-left text-[11px] transition-opacity ${
                  dimmed
                    ? "text-(--color-text-muted) opacity-40"
                    : "text-(--color-text-secondary) hover:bg-white/5"
                }`}
              >
                <span
                  className="inline-block h-2.5 w-2.5 shrink-0 rounded-full ring-1 ring-black/20"
                  style={{ background: LEGEND_COLORS[key] }}
                  aria-hidden
                />
                <span className={active && filtered ? "font-medium text-(--color-text-primary)" : undefined}>
                  {legendLabel(key, locale)}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
    </aside>
  );
}

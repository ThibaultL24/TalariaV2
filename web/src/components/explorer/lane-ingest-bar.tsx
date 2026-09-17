// web/src/components/explorer/lane-ingest-bar.tsx
import { useI18n } from "@/lib/i18n";

interface LaneIngestBarProps {
  lane: "explorer" | "agora";
  busy?: boolean;
  status?: string | null;
  disabled?: boolean;
  onRun: () => void;
}

export function LaneIngestBar({
  lane,
  busy,
  status,
  disabled,
  onRun,
}: LaneIngestBarProps) {
  const { t } = useI18n();
  const isExplorer = lane === "explorer";
  const label = isExplorer ? t.collectLifeTrace : t.collectScholarship;
  const hint = isExplorer ? t.collectLifeTraceHint : t.collectScholarshipHint;

  return (
    <div className="border-b border-(--color-border-subtle) px-3 py-2">
      <button
        type="button"
        onClick={onRun}
        disabled={disabled || busy}
        className="w-full rounded-lg border border-(--color-border-subtle) bg-(--color-bg-surface) px-3 py-2 text-sm font-medium text-(--color-text-primary) disabled:opacity-40"
      >
        {busy ? t.collecting : label}
      </button>
      <p className="mt-1.5 text-[10px] leading-relaxed text-(--color-text-muted)">
        {status ?? hint}
      </p>
    </div>
  );
}

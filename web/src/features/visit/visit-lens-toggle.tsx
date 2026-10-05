// web/src/features/visit/visit-lens-toggle.tsx
import { useNavigate, useSearchParams } from "react-router-dom";
import { useI18n } from "@/lib/i18n";

export type ExplorerLens = "scholar" | "visit";

export function useExplorerLens(): ExplorerLens {
  const [params] = useSearchParams();
  return params.get("lens") === "visit" ? "visit" : "scholar";
}

interface VisitLensToggleProps {
  entityId: string;
  view: string;
}

export function VisitLensToggle({ entityId, view }: VisitLensToggleProps) {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const lens = useExplorerLens();

  function setLens(next: ExplorerLens) {
    const copy = new URLSearchParams(params);
    if (next === "scholar") copy.delete("lens");
    else copy.set("lens", "visit");
    const nextView =
      next === "visit"
        ? ["map", "heritage", "now"].includes(view)
          ? view
          : "map"
        : ["overview", "timeline", "map", "sources"].includes(view)
          ? view
          : "overview";
    navigate(`/entities/${entityId}/${nextView}?${copy.toString()}`);
  }

  return (
    <div className="visit-lens-toggle" role="group" aria-label={t.lensToggleLabel}>
      <button
        type="button"
        className={lens === "scholar" ? "is-active" : ""}
        aria-pressed={lens === "scholar"}
        onClick={() => setLens("scholar")}
      >
        {t.lensScholar}
      </button>
      <button
        type="button"
        className={lens === "visit" ? "is-active" : ""}
        aria-pressed={lens === "visit"}
        onClick={() => setLens("visit")}
      >
        {t.lensVisit}
      </button>
    </div>
  );
}

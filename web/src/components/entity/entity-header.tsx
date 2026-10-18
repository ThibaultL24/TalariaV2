// web/src/components/entity/entity-header.tsx
import { Link } from "react-router-dom";
import { TalariaSandalMark } from "@/components/brand/talaria-sandal-mark";
import { experienceHref, type LifeExperience } from "@/lib/experiences";
import { useI18n } from "@/lib/i18n";

interface EntityHeaderProps {
  experience: LifeExperience;
  entityId?: string;
  label?: string;
}

export function EntityHeader({ experience, entityId, label }: EntityHeaderProps) {
  const { t } = useI18n();
  const isPoreia = experience === "poreia";
  const title = label ?? (entityId ? t.loading : isPoreia ? "Poreia" : "Istoria");
  const other = isPoreia ? "istoria" : "poreia";

  return (
    <div className="v3-heading">
      <div>
        <p className="v3-eyebrow">TALARIA · {isPoreia ? "POREIA" : "ISTORIA"}</p>
        <h1>
          <span className="talaria-heading-with-mark">
            <TalariaSandalMark />
            {title}
          </span>
        </h1>
        <p>{isPoreia ? t.poreiaPromise : t.istoriaPromise}</p>
        {entityId ? (
          <div className="talaria-experience-links">
            <Link className="button button--ghost" to={experienceHref(other, entityId)}>
              {isPoreia ? t.openIstoria : t.openPoreia}
            </Link>
          </div>
        ) : null}
      </div>
    </div>
  );
}

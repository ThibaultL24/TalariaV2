// web/src/components/entity/entity-header.tsx
import { Link } from "react-router-dom";
import { TalariaSandalMark } from "@/components/brand/talaria-sandal-mark";
import { experienceHref, type LifeExperience } from "@/lib/experiences";

interface EntityHeaderProps {
  experience: LifeExperience;
  entityId?: string;
  label?: string;
}

export function EntityHeader({ experience, entityId, label }: EntityHeaderProps) {
  const isPoreia = experience === "poreia";
  const title = label ?? (entityId ? "Loading…" : isPoreia ? "Poreia" : "Istoria");
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
        <p>
          {isPoreia
            ? "Le voyage culturel : lieux, monuments et parcours."
            : "L’enquête historique : faits, chronologie et sources."}
        </p>
        {entityId ? (
          <div className="talaria-experience-links">
            <Link className="button button--ghost" to={experienceHref(other, entityId)}>
              {isPoreia ? "Explorer sa vie dans Istoria →" : "Découvrir ses lieux dans Poreia →"}
            </Link>
          </div>
        ) : null}
      </div>
    </div>
  );
}

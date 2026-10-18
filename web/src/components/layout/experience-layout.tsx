// web/src/components/layout/experience-layout.tsx
import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import { EntityHeader } from "@/components/entity/entity-header";
import { ExperienceDemoFigures } from "@/components/layout/experience-demo-figures";
import { ExperienceNavigation } from "@/components/layout/experience-navigation";
import { ExperienceSubnav } from "@/components/layout/experience-subnav";
import { Navbar } from "@/components/layout/navbar";
import type { LifeExperience } from "@/lib/experiences";
import { useI18n } from "@/lib/i18n";

interface ExperienceLayoutProps {
  experience: LifeExperience;
  entityId: string;
  label?: string;
  immersive: boolean;
  mobileStage: boolean;
  toolsOpen: boolean;
  onToggleTools: () => void;
  showHeroBand: boolean;
  search: ReactNode;
  children: ReactNode;
}

export function ExperienceLayout({
  experience,
  entityId,
  label,
  immersive,
  mobileStage,
  toolsOpen,
  onToggleTools,
  showHeroBand,
  search,
  children,
}: ExperienceLayoutProps) {
  const { t } = useI18n();
  const shell = [
    "v3-shell",
    `v3-shell--${experience}`,
    immersive ? "v3-shell--immersive" : "",
    experience === "istoria" && immersive ? "v3-shell--scholar-stage" : "",
    mobileStage ? "v3-shell--mobile-stage" : "",
  ].filter(Boolean).join(" ");

  return (
    <div className={shell}>
      {mobileStage ? (
        <header className="v3-mobile-chrome">
          <Link className="v3-mobile-chrome__back" to={`/${experience}/${entityId}/${experience === "poreia" ? "map" : "overview"}`}>
            {t.immersiveBack}
          </Link>
          <h1 className="v3-mobile-chrome__title">{label ?? "Loading…"}</h1>
          <button
            type="button"
            className="v3-mobile-chrome__tools"
            aria-expanded={toolsOpen}
            onClick={onToggleTools}
          >
            {t.immersiveTools}
          </button>
        </header>
      ) : (
        <Navbar />
      )}
      <main className={`v3-main${immersive ? " v3-main--immersive" : ""}`}>
        {!immersive && <ExperienceNavigation entityId={entityId || undefined} compact={Boolean(entityId)} />}
        {!immersive && (
          <section className={showHeroBand ? "v3-overview-hero" : undefined}>
            <EntityHeader experience={experience} entityId={entityId || undefined} label={label} />
            <section className="v3-search" aria-label="Search a person">
              {search}
            </section>
          </section>
        )}
        {!entityId && <ExperienceDemoFigures experience={experience} />}
        {immersive && entityId && !mobileStage && (
          <div className="v3-immersive-bar">
            <h1 className="v3-immersive-title">{label ?? "Loading…"}</h1>
            <span className="talaria-experience__subtitle">
              {experience === "istoria" ? "Faits & anecdotes" : "Tourisme patrimonial"}
            </span>
          </div>
        )}
        {entityId && !immersive && (
          <div className="v3-lens-row">
            <span className="talaria-experience__subtitle">
              {experience === "istoria" ? "Faits & anecdotes" : "Tourisme patrimonial"}
            </span>
          </div>
        )}
        {entityId ? <ExperienceSubnav experience={experience} entityId={entityId} /> : null}
        {children}
      </main>
    </div>
  );
}

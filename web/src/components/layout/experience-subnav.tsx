// web/src/components/layout/experience-subnav.tsx
import { NavLink, useSearchParams } from "react-router-dom";
import { experienceSectionHref, experienceTabs, type ExperienceId } from "@/lib/experiences";
import { useI18n } from "@/lib/i18n";

interface ExperienceSubnavProps {
  experience: ExperienceId;
  entityId: string;
}

const SECTION_LABEL = {
  fr: { istoria: "Sections Istoria", poreia: "Sections Poreia", agora: "Sections Agora" },
  en: { istoria: "Istoria sections", poreia: "Poreia sections", agora: "Agora sections" },
} as const;

export function ExperienceSubnav({ experience, entityId }: ExperienceSubnavProps) {
  const { locale } = useI18n();
  const [params] = useSearchParams();
  const tabs = experienceTabs(experience, locale);

  return (
    <nav className="v3-tabs experience-subnav" aria-label={SECTION_LABEL[locale][experience]}>
      {tabs.map((tab) => (
        <NavLink
          key={tab.id}
          to={experienceSectionHref(experience, entityId, tab.id, params.toString())}
          className={({ isActive }) => (isActive ? "active" : undefined)}
        >
          {tab.label}
        </NavLink>
      ))}
    </nav>
  );
}

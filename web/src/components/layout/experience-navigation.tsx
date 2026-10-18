import { NavLink, useLocation } from "react-router-dom";
import { useI18n } from "@/lib/i18n";

type Experience = "istoria" | "poreia" | "agora";

interface ExperienceNavigationProps {
  entityId?: string;
  compact?: boolean;
}

const EXPERIENCES: ReadonlyArray<{
  id: Experience;
  name: string;
  greek: string;
  meaning: { fr: string; en: string };
  promise: { fr: string; en: string };
}> = [
  {
    id: "istoria",
    name: "Istoria",
    greek: "ἱστορία",
    meaning: { fr: "Enquêter sur le passé", en: "Investigate the past" },
    promise: { fr: "Faits, frise et sources", en: "Facts, timeline and sources" },
  },
  {
    id: "poreia",
    name: "Poreia",
    greek: "πορεία",
    meaning: { fr: "Parcourir les lieux", en: "Discover the places" },
    promise: { fr: "Sites, carte et parcours", en: "Sites, map and journeys" },
  },
  {
    id: "agora",
    name: "Agora",
    greek: "ἀγορά",
    meaning: { fr: "Confronter les idées", en: "Debate interpretations" },
    promise: { fr: "Thèses et discussions", en: "Claims and discussion" },
  },
];

export function ExperienceNavigation({ entityId, compact = false }: ExperienceNavigationProps) {
  const { locale } = useI18n();
  const { pathname } = useLocation();
  const href = (id: Experience): string => {
    if (id === "agora") return "/agora";
    if (!entityId) return `/${id}`;
    return id === "poreia" ? `/poreia/${entityId}/map` : `/istoria/${entityId}/overview`;
  };

  return (
    <nav
      className={`experience-navigation${compact ? " experience-navigation--compact" : ""}`}
      aria-label={locale === "fr" ? "Les trois univers de Talaria" : "Talaria's three experiences"}
    >
      <div className="experience-navigation__heading">
        <span className="experience-navigation__eyebrow">TALARIA</span>
        <span>{locale === "fr" ? "Trois façons de découvrir l’histoire" : "Three ways to experience history"}</span>
      </div>
      <div className="experience-navigation__items">
        {EXPERIENCES.map((experience, index) => {
          const isActive = pathname === `/${experience.id}` || pathname.startsWith(`/${experience.id}/`);
          return (
            <NavLink
              key={experience.id}
              to={href(experience.id)}
              end={false}
              className={`experience-navigation__item experience-navigation__item--${experience.id}${isActive ? " is-current" : ""}`}
              aria-current={isActive ? "page" : undefined}
            >
              <span className="experience-navigation__index">0{index + 1}</span>
              <span className="experience-navigation__copy">
                <strong>{experience.name}</strong>
                <span className="experience-navigation__meaning">{experience.greek} · {experience.meaning[locale]}</span>
                {!compact && <small>{experience.promise[locale]}</small>}
              </span>
              <span className="experience-navigation__arrow" aria-hidden="true">↗</span>
            </NavLink>
          );
        })}
      </div>
    </nav>
  );
}

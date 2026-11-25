// web/src/components/layout/experience-navigation.tsx
import { Link, useLocation } from "react-router-dom";
import { experienceHref, type ExperienceId } from "@/lib/experiences";
import { useI18n } from "@/lib/i18n";

interface ExperienceNavigationProps {
  entityId?: string;
  compact?: boolean;
  subjectLabel?: string;
}

const EXPERIENCES: ReadonlyArray<{
  id: ExperienceId;
  name: string;
  greek: string;
  meaning: { fr: string; en: string };
  promise: { fr: string; en: string };
  rail: { fr: string; en: string };
}> = [
  {
    id: "istoria",
    name: "Istoria",
    greek: "ἱστορία",
    meaning: { fr: "Enquêter sur le passé", en: "Investigate the past" },
    promise: { fr: "Faits, frise et sources", en: "Facts, timeline and sources" },
    rail: { fr: "Sa vie · Istoria", en: "Their life · Istoria" },
  },
  {
    id: "poreia",
    name: "Poreia",
    greek: "πορεία",
    meaning: { fr: "Parcourir les lieux", en: "Discover the places" },
    promise: { fr: "Sites, carte et parcours", en: "Sites, map and journeys" },
    rail: { fr: "Son héritage · Poreia", en: "Their heritage · Poreia" },
  },
  {
    id: "agora",
    name: "Agora",
    greek: "ἀγορά",
    meaning: { fr: "Confronter les idées", en: "Debate interpretations" },
    promise: { fr: "Thèses et discussions", en: "Claims and discussion" },
    rail: { fr: "Ses débats · Agora", en: "Their debates · Agora" },
  },
];

export function ExperienceNavigation({ entityId, compact = false, subjectLabel }: ExperienceNavigationProps) {
  const { locale } = useI18n();
  const { pathname } = useLocation();
  const href = (id: ExperienceId): string => experienceHref(id, entityId);
  const subject = subjectLabel?.trim();

  if (compact && subject) {
    return (
      <nav
        className="subject-rail"
        aria-label={locale === "fr" ? "Les trois univers de Talaria" : "Talaria's three experiences"}
      >
        <div className="subject-rail__name">
          <span className="subject-rail__dot" aria-hidden="true" />
          <span>
            <small>{locale === "fr" ? "Votre exploration" : "Your exploration"}</small>
            <strong>{subject}</strong>
          </span>
        </div>
        <div className="subject-rail__links">
          {EXPERIENCES.map((experience) => {
            const isActive = pathname === `/${experience.id}` || pathname.startsWith(`/${experience.id}/`);
            return (
              <Link
                key={experience.id}
                to={href(experience.id)}
                className={isActive ? "is-current" : undefined}
                aria-current={isActive ? "page" : undefined}
              >
                {experience.rail[locale]}
              </Link>
            );
          })}
        </div>
      </nav>
    );
  }

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
            <Link
              key={experience.id}
              to={href(experience.id)}
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
            </Link>
          );
        })}
      </div>
    </nav>
  );
}

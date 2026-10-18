import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { fetchDemoRoster, type DemoRosterItem } from "@/lib/api";
import { DEMO_ROSTER } from "@/lib/demo-roster";
import { useI18n } from "@/lib/i18n";

export function ExperienceDemoFigures({ experience }: { experience: "istoria" | "poreia" }) {
  const { locale } = useI18n();
  const [known, setKnown] = useState<Map<string, DemoRosterItem>>(new Map());
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");

  useEffect(() => {
    let cancelled = false;
    void fetchDemoRoster().then((response) => {
      if (cancelled) return;
      setKnown(new Map(response.items.map((item) => [item.qid, item])));
      setStatus("ready");
    }).catch(() => {
      if (!cancelled) setStatus("error");
    });
    return () => { cancelled = true; };
  }, []);

  const available = DEMO_ROSTER.flatMap((person) => {
    const entry = known.get(person.qid);
    if (!entry?.entity_id || !entry.known_locally) return [];
    return [{...person, entityId: entry.entity_id}];
  });

  return (
    <section className="experience-demo-figures" aria-labelledby="experience-demo-figures-title">
      <header className="experience-demo-figures__head">
        <p className="experience-section-kicker">
          {locale === "fr" ? "DÉCOUVRIR" : "DISCOVER"} · {experience.toUpperCase()}
        </p>
        <h2 id="experience-demo-figures-title">
          {locale === "fr" ? "Commencer par une personnalité" : "Start with a historical figure"}
        </h2>
        <p>{experience === "istoria"
          ? (locale === "fr" ? "Explorez une vie à travers les faits, les lieux et les sources." : "Explore a life through facts, places and sources.")
          : (locale === "fr" ? "Découvrez les monuments et lieux de mémoire documentés autour d’une personnalité." : "Discover documented monuments and heritage places connected to a person.")}</p>
      </header>
      {status === "loading" && <p role="status">{locale === "fr" ? "Chargement des figures…" : "Loading figures…"}</p>}
      {status === "error" && <p role="alert">{locale === "fr" ? "Catalogue indisponible pour le moment." : "Catalog currently unavailable."}</p>}
      {status === "ready" && available.length === 0 && <p role="status">
        {locale === "fr" ? "Aucune figure de démonstration disponible." : "No demo figures available."}
      </p>}
      {available.length > 0 && (
        <ul className="experience-demo-figures__grid">
          {available.map((person) => (
            <li key={person.qid}>
              <Link to={experience === "istoria"
                ? `/istoria/${person.entityId}/overview`
                : `/poreia/${person.entityId}/heritage`}>
                <span className="experience-demo-figures__monogram" aria-hidden="true">
                  {person.labelFr.slice(0,1)}
                </span>
                <span className="experience-demo-figures__name">
                  {locale === "fr" ? person.labelFr : person.labelEn}
                  <small>{experience === "istoria"
                    ? (locale === "fr" ? "Voir sa vie et ses sources" : "Explore their life and sources")
                    : (locale === "fr" ? "Découvrir les lieux liés" : "Explore related places")}</small>
                </span>
                <span aria-hidden="true">↗</span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

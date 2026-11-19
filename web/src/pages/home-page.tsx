// web/src/pages/home-page.tsx
import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { Navbar } from "@/components/layout/navbar";
import { EntitySearchBox } from "@/components/search/entity-search-box";
import { usePersonPicker } from "@/hooks/use-person-picker";
import { fetchDemoRoster, type DemoRosterItem } from "@/lib/api";
import { DEMO_ROSTER, type DemoRosterEntry } from "@/lib/demo-roster";
import { experienceHref } from "@/lib/experiences";
import { useI18n } from "@/lib/i18n";
import type { SearchSuggestion } from "@/lib/schemas/entity";
import { useExplorerStore } from "@/stores/explorer-store";

interface RosterCard extends DemoRosterEntry {
  stats?: DemoRosterItem;
}

export function HomePage() {
  const { t, locale } = useI18n();
  const navigate = useNavigate();
  const { setEntity, setPersonFilter, entityId: heldEntityId } = useExplorerStore();
  const { suggestions, setSearchQuery, searchLoading, selectPerson } = usePersonPicker({ startLifeIngest: false });
  const [roster, setRoster] = useState<RosterCard[]>(() =>
    DEMO_ROSTER.map((entry) => ({ ...entry })),
  );

  useEffect(() => {
    let cancelled = false;
    fetchDemoRoster()
      .then((payload) => {
        if (cancelled) return;
        const byQid = new Map(payload.items.map((item) => [item.qid, item]));
        setRoster(
          DEMO_ROSTER.map((entry) => ({
            ...entry,
            stats: byQid.get(entry.qid),
          })),
        );
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  function onSelect(item: SearchSuggestion) {
    const entityId = selectPerson(item);
    navigate(entityId ? experienceHref("istoria", entityId) : "/istoria");
  }

  function openFigure(entry: RosterCard, lane: "explorer" | "agora") {
    const label = locale === "fr" ? entry.labelFr : entry.labelEn;
    if (lane === "agora") {
      if (entry.stats?.entity_id) {
        setEntity(entry.stats.entity_id, label, entry.qid);
      } else {
        setPersonFilter(label, label, entry.qid);
      }
      navigate(entry.stats?.entity_id ? experienceHref("agora", entry.stats.entity_id) : "/agora");
      return;
    }
    if (entry.stats?.entity_id) {
      setEntity(entry.stats.entity_id, label, entry.qid);
      navigate(`/istoria/${entry.stats.entity_id}/overview`);
      return;
    }
    setPersonFilter(label, label, entry.qid);
    navigate("/istoria");
  }

  const fr = locale === "fr";
  const lifeHref = experienceHref("istoria", heldEntityId ?? undefined);
  const placeHref = experienceHref("poreia", heldEntityId ?? undefined);
  const ideaHref = experienceHref("agora", heldEntityId ?? undefined);
  const featured = roster.find((entry) => entry.qid === "Q517");
  const featuredLabel = featured ? (fr ? featured.labelFr : featured.labelEn) : null;
  const featuredReady = Boolean(featured?.stats?.entity_id && (featured.stats.event_count ?? 0) > 0);

  return (
    <div className="app-shell app-shell--home">
      <Navbar />
      <main className="home-v43">
        <section className="v43-hero" aria-labelledby="home-hero-title">
          <div className="v43-hero__plate" aria-hidden="true">
            <img className="v43-hero__sea" alt="" src="/brand/hero-aegean-day.jpg" />
            <img className="v43-hero__fresco" alt="" src="/media/home/school-of-athens.jpg" />
          </div>
          <div className="v43-hero__copy">
            <p className="v43-eyebrow">
              <img alt="" className="v43-sandal" src="/brand/talaria-sandal.png" />
              {fr ? "Méditerranée · Culture · Mémoire" : "Mediterranean · Culture · Memory"}
            </p>
            <h1 id="home-hero-title">
              {fr ? <>Le temps.<br />Les lieux. Les idées.</> : <>Time.<br />Places. Ideas.</>}
            </h1>
            <p>
              {fr
                ? "Un musée vivant. Choisissez une personnalité, puis traversez sa vie, son héritage et ses idées sans perdre le fil."
                : "A living museum. Choose a person, then move through their life, their heritage and their ideas without losing the thread."}
            </p>
            <div className="v43-search">
              <EntitySearchBox
                suggestions={suggestions}
                onSubmitQuery={setSearchQuery}
                onSelect={onSelect}
                isLoading={searchLoading}
              />
            </div>
            <div className="v43-quick">
              <Link to={lifeHref}>{fr ? "Sa vie" : "Their life"}</Link>
              <Link to={placeHref}>{fr ? "Son patrimoine" : "Their heritage"}</Link>
              <Link to={ideaHref}>{fr ? "Ses idées et ses débats" : "Their ideas and debates"}</Link>
            </div>
            <p className="v43-credit">
              {fr
                ? "La boucle filmée n’est pas encore déposée. Le fond assemble une vue méditerranéenne du projet et L’École d’Athènes de Raphaël, domaine public. Ce n’est pas une archive de survol."
                : "The filmed loop is not in place yet. The background joins a project view of the Mediterranean with Raphael’s School of Athens, public domain. It is not a flyover archive."}
            </p>
          </div>
        </section>

        <section className="v43-wrap" aria-labelledby="home-visions-title">
          <header className="v43-head">
            <div>
              <p className="v43-eyebrow">{fr ? "Trois regards sur le monde" : "Three ways of looking"}</p>
              <h2 id="home-visions-title">{fr ? "Choisissez votre voyage." : "Choose the journey."}</h2>
            </div>
            <p>{t.homeVisionsHint}</p>
          </header>
          <div className="v43-worlds">
            <Link className="v43-world" to={lifeHref} style={{ ["--art" as string]: "url(/media/home/david-alps.jpg)" }}>
              <span className="v43-eyebrow">01 / {fr ? "Le temps" : "Time"}</span>
              <strong>Istoria</strong>
              <em>{fr ? "Vies, événements et siècles" : "Lives, events and centuries"}</em>
            </Link>
            <Link className="v43-world" to={placeHref} style={{ ["--art" as string]: "url(/media/home/invalides-dome.jpg)" }}>
              <span className="v43-eyebrow">02 / {fr ? "Les lieux" : "Places"}</span>
              <strong>Poreia</strong>
              <em>{fr ? "Musées, lieux de mémoire et agenda culturel" : "Museums, memory places and a cultural calendar"}</em>
            </Link>
            <Link className="v43-world" to={ideaHref} style={{ ["--art" as string]: "url(/media/home/school-of-athens.jpg)" }}>
              <span className="v43-eyebrow">03 / {fr ? "Les idées" : "Ideas"}</span>
              <strong>Agora</strong>
              <em>{fr ? "Cours, textes, sources et discussions" : "Courses, texts, sources and discussion"}</em>
            </Link>
          </div>
        </section>

        <section className="v43-wrap v43-feature">
          <figure className="v43-feature__art">
            <img alt={fr ? "Jacques-Louis David, Bonaparte franchissant le Grand-Saint-Bernard" : "Jacques-Louis David, Bonaparte crossing the Alps"} src="/media/home/david-alps.jpg" />
            <figcaption>
              {fr
                ? "Jacques-Louis David, Bonaparte franchissant le Grand-Saint-Bernard, domaine public. Illustration d’accueil, pas une preuve du corpus."
                : "Jacques-Louis David, Bonaparte crossing the Alps, public domain. A welcome image, not evidence from the corpus."}
            </figcaption>
          </figure>
          <div>
            <p className="v43-eyebrow">{fr ? "Un récit à découvrir" : "A life to open"}</p>
            <h2>{featuredLabel ?? (fr ? "Une vie déjà préparée" : "A prepared life")}</h2>
            <p>
              {featuredReady && featured?.stats
                ? t.demoRosterStats(featured.stats.event_count, featured.stats.map_pin_count, featured.stats.claim_count)
                : (fr ? "Le récit s’ouvrira quand cette personnalité sera dans le corpus." : "The account opens once this person is in the corpus.")}
            </p>
            <button type="button" className="button button--primary" onClick={() => featured && openFigure(featured, "explorer")}>
              {fr ? "Entrer dans Istoria" : "Enter Istoria"}
            </button>
          </div>
        </section>

        <section className="v43-wrap" aria-label={fr ? "Salle d’images" : "Picture room"}>
          <ul className="v43-gallery">
            <li>
              <img alt={fr ? "Dôme des Invalides, au-dessus du tombeau de Napoléon" : "Dome of Les Invalides, above Napoleon’s tomb"} src="/media/home/invalides-dome.jpg" />
              <span>{fr ? "Les Invalides · Wikimedia Commons" : "Les Invalides · Wikimedia Commons"}</span>
            </li>
            <li>
              <img alt={fr ? "Cathédrale Santa Maria del Fiore, Florence" : "Florence Cathedral, Santa Maria del Fiore"} src="/media/home/florence-duomo.jpg" />
              <span>{fr ? "Florence, Santa Maria del Fiore · Wikimedia Commons" : "Florence, Santa Maria del Fiore · Wikimedia Commons"}</span>
            </li>
            <li>
              <img alt={fr ? "Raphaël, L’École d’Athènes" : "Raphael, The School of Athens"} src="/media/home/school-of-athens.jpg" />
              <span>{fr ? "Raphaël · L’École d’Athènes, domaine public" : "Raphael · The School of Athens, public domain"}</span>
            </li>
          </ul>
        </section>

        <section className="demo-roster" aria-labelledby="demo-roster-title">
          <div className="demo-roster__inner">
            <header className="demo-roster__header">
              <h2 id="demo-roster-title" className="demo-roster__title">
                {t.demoRosterTitle}
              </h2>
              <p className="demo-roster__hint">{t.demoRosterHint}</p>
            </header>
            <ul className="demo-roster__grid">
              {roster.map((entry) => {
                const label = locale === "fr" ? entry.labelFr : entry.labelEn;
                const events = entry.stats?.event_count ?? 0;
                const pins = entry.stats?.map_pin_count ?? 0;
                const claims = entry.stats?.claim_count ?? 0;
                const ready = Boolean(entry.stats?.known_locally && events > 0);
                return (
                  <li key={entry.qid} className="demo-roster__card">
                    <div className="demo-roster__card-top">
                      <span
                        className={`demo-roster__era ${
                          entry.era === "modern"
                            ? "demo-roster__era--modern"
                            : "demo-roster__era--historical"
                        }`}
                      >
                        {entry.era === "modern" ? t.demoEraModern : t.demoEraHistorical}
                      </span>
                      {entry.intuitionStar ? (
                        <span className="demo-roster__star">{t.demoIntuitionStar}</span>
                      ) : null}
                    </div>
                    <h3 className="demo-roster__name">{label}</h3>
                    <p className="demo-roster__stats">
                      {ready
                        ? t.demoRosterStats(events, pins, claims)
                        : t.demoRosterPending}
                    </p>
                    <div className="demo-roster__actions">
                      <button
                        type="button"
                        className="button button--primary demo-roster__btn"
                        onClick={() => openFigure(entry, "explorer")}
                      >
                        {t.explorer}
                      </button>
                      <button
                        type="button"
                        className="button button--ghost demo-roster__btn"
                        onClick={() => openFigure(entry, "agora")}
                      >
                        {t.agora}
                      </button>
                    </div>
                  </li>
                );
              })}
            </ul>
          </div>
        </section>

        <section className="home-about" id="about" aria-labelledby="home-about-title">
          <div className="home-about__inner">
            <header className="home-about__header">
              <h2 id="home-about-title" className="home-about__title">
                {t.homeAboutTitle}
              </h2>
              <div className="home-about__title-accent" aria-hidden />
            </header>
            <ul className="home-about__pillars">
              <li>
                <span className="home-about__pillar-label">{t.homeAboutDoctrine}</span>
                <span className="home-about__pillar-text">{t.homeAboutDoctrineBody}</span>
              </li>
              <li>
                <span className="home-about__pillar-label">{t.homeAboutSeparates}</span>
                <span className="home-about__pillar-text">{t.homeAboutSeparatesBody}</span>
              </li>
              <li>
                <span className="home-about__pillar-label">{t.homeAboutFreedom}</span>
                <span className="home-about__pillar-text">{t.homeAboutFreedomBody}</span>
              </li>
            </ul>
            <p className="home-about__whitepaper">
              <Link to="/about">{t.homeAboutWhitepaper}</Link>
            </p>
          </div>
        </section>

        <footer className="home-footer">
          <nav className="home-footer__nav" aria-label={locale === "fr" ? "Pied de page" : "Footer"}>
            <Link to="/istoria">Istoria</Link>
            <Link to="/poreia">Poreia</Link>
            <Link to="/agora">Agora</Link>
            <Link to="/about">{locale === "fr" ? "Qui sommes-nous ?" : "About"}</Link>
          </nav>
          <p className="home-footer__note">
            {locale === "fr"
              ? "La sandale ailée reste l’emblème. Les photographies d’accueil sont des vues méditerranéennes du projet, pas des archives historiques."
              : "The winged sandal remains the emblem. The home photographs are Mediterranean views from the project, not historical archives."}
          </p>
        </footer>
      </main>
    </div>
  );
}

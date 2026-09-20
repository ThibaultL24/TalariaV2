// web/src/pages/home-page.tsx
import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { Navbar } from "@/components/layout/navbar";
import { EntitySearchBox } from "@/components/search/entity-search-box";
import { usePersonPicker } from "@/hooks/use-person-picker";
import { fetchDemoRoster, type DemoRosterItem } from "@/lib/api";
import { DEMO_ROSTER, type DemoRosterEntry } from "@/lib/demo-roster";
import { useI18n } from "@/lib/i18n";
import type { SearchSuggestion } from "@/lib/schemas/entity";
import { useExplorerStore } from "@/stores/explorer-store";

interface RosterCard extends DemoRosterEntry {
  stats?: DemoRosterItem;
}

export function HomePage() {
  const { t, locale } = useI18n();
  const navigate = useNavigate();
  const { setEntity, setPersonFilter } = useExplorerStore();
  const { suggestions, setSearchQuery, searchLoading, selectPerson } = usePersonPicker();
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
    selectPerson(item);
    navigate("/explorer");
  }

  function openFigure(entry: RosterCard, lane: "explorer" | "agora") {
    const label = locale === "fr" ? entry.labelFr : entry.labelEn;
    if (entry.stats?.entity_id) {
      setEntity(entry.stats.entity_id, label, entry.qid);
    } else {
      setPersonFilter(label, label, entry.qid);
    }
    navigate(lane === "agora" ? "/agora" : "/explorer");
  }

  return (
    <div className="app-shell app-shell--home">
      <Navbar />
      <main className="home-main">
        <section className="hero hero--landing hero--home" aria-labelledby="home-hero-title">
          <div className="hero__grid">
            <div className="hero__copy">
              <p className="hero__eyebrow">{t.heroEyebrow}</p>
              <h1 id="home-hero-title" className="hero__title">
                {t.productName}
              </h1>
              <p className="hero__subtitle">{t.heroSubtitle}</p>
              <div className="mx-auto mt-6 w-full max-w-xl">
                <EntitySearchBox
                  suggestions={suggestions}
                  onSubmitQuery={setSearchQuery}
                  onSelect={onSelect}
                  isLoading={searchLoading}
                />
              </div>
              <div className="hero__cta">
                <Link className="button button--primary hero__cta-primary" to="/explorer">
                  {t.startExploration}
                </Link>
                <Link className="button button--ghost hero__cta-secondary" to="/agora">
                  {t.openAgora}
                </Link>
              </div>
            </div>
            <aside className="hero__aside" aria-label={t.productSubtitle}>
              <div className="hero__features">
                <article className="hero__feature">
                  <span className="hero__feature-mark" aria-hidden />
                  <div className="hero__feature-body">
                    <h3 className="hero__feature-title">{t.livingMap}</h3>
                    <p className="hero__feature-text">{t.livingMapDesc}</p>
                  </div>
                </article>
                <article className="hero__feature">
                  <span className="hero__feature-mark hero__feature-mark--soft" aria-hidden />
                  <div className="hero__feature-body">
                    <h3 className="hero__feature-title">{t.agora}</h3>
                    <p className="hero__feature-text">{t.agoraHint}</p>
                  </div>
                </article>
              </div>
            </aside>
          </div>
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
      </main>
    </div>
  );
}

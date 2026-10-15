// web/src/pages/home-page.tsx
import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { Navbar } from "@/components/layout/navbar";
import { EntitySearchBox } from "@/components/search/entity-search-box";
import { usePersonPicker } from "@/hooks/use-person-picker";
import { fetchDemoRoster, type DemoRosterItem } from "@/lib/api";
import { DEMO_ROSTER, FEATURED_ROSTER_QIDS, type DemoRosterEntry } from "@/lib/demo-roster";
import { fetchWikipediaPageSummary } from "@/lib/wikipedia";
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
  const { suggestions, setSearchQuery, searchLoading } = usePersonPicker({ startLifeIngest: false });
  const [roster, setRoster] = useState<RosterCard[]>(() =>
    DEMO_ROSTER.map((entry) => ({ ...entry })),
  );
  const [portraits, setPortraits] = useState<Record<string, { url: string; page: string }>>({});

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

  useEffect(() => {
    let cancelled = false;
    const featured = DEMO_ROSTER.filter((entry) =>
      (FEATURED_ROSTER_QIDS as readonly string[]).includes(entry.qid),
    );
    void Promise.all(
      featured.map(async (entry) => {
        const summary = await fetchWikipediaPageSummary("en", entry.wikiTitle);
        return [
          entry.qid,
          summary?.thumbnailSource
            ? { url: summary.thumbnailSource, page: summary.url ?? "" }
            : null,
        ] as const;
      }),
    ).then((pairs) => {
      if (cancelled) return;
      setPortraits(
        Object.fromEntries(
          pairs.flatMap(([qid, portrait]) => (portrait ? [[qid, portrait]] : [])),
        ),
      );
    });
    return () => {
      cancelled = true;
    };
  }, []);

  function onSelect(item: SearchSuggestion) {
    const qid = item.qid ? `&qid=${encodeURIComponent(item.qid)}` : "";
    const subject = encodeURIComponent(item.label);
    if (item.known_locally && item.entity_id) {
      navigate(`/entities/${item.entity_id}/overview`);
      return;
    }
    navigate(`/explorer?subject=${subject}${qid}`);
  }

  function openFigure(entry: RosterCard, lane: "explorer" | "agora") {
    const label = locale === "fr" ? entry.labelFr : entry.labelEn;
    if (lane === "agora") {
      if (entry.stats?.entity_id) setEntity(entry.stats.entity_id, label, entry.qid);
      else setPersonFilter(label, label, entry.qid);
      navigate("/agora");
      return;
    }
    if (entry.stats?.entity_id) {
      navigate(`/entities/${entry.stats.entity_id}/overview`);
      return;
    }
    navigate(`/explorer?subject=${encodeURIComponent(label)}${entry.qid ? `&qid=${encodeURIComponent(entry.qid)}` : ""}`);
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

        <section className="home-lanes" aria-label={t.productName}>
          <ul className="home-lanes__list">
            <li>
              <Link to="/scholar">
                <strong>{t.productLaneScholarTitle}</strong>
                <span>{t.productLaneScholarBody}</span>
              </Link>
            </li>
            <li>
              <Link to="/visit">
                <strong>{t.productLaneVisitTitle}</strong>
                <span>{t.productLaneVisitBody}</span>
              </Link>
            </li>
            <li>
              <Link to="/agora">
                <strong>{t.productLaneAgoraTitle}</strong>
                <span>{t.productLaneAgoraBody}</span>
              </Link>
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
            <ul className="demo-roster__featured">
              {roster
                .filter((entry) => (FEATURED_ROSTER_QIDS as readonly string[]).includes(entry.qid))
                .map((entry) => {
                  const label = locale === "fr" ? entry.labelFr : entry.labelEn;
                  const portrait = portraits[entry.qid];
                  return (
                    <li key={entry.qid} className="featured-figure">
                      <Link className="featured-figure__main" to={`/scholar/${entry.qid}`}>
                        {portrait ? (
                          <img src={portrait.url} alt="" width={160} height={200} loading="lazy" />
                        ) : (
                          <span className="featured-figure__fallback" aria-hidden />
                        )}
                        <span className="featured-figure__copy">
                          <h3>{label}</h3>
                          <span className="featured-figure__cta">{t.exploreLife}</span>
                        </span>
                      </Link>
                      <div className="featured-figure__lanes">
                        {portrait?.page ? (
                          <a href={portrait.page} rel="noopener noreferrer">
                            {t.imageCredit}
                          </a>
                        ) : null}
                        <Link to={`/visit/${entry.qid}`}>{t.discoverPlaces}</Link>
                        <button type="button" onClick={() => openFigure(entry, "agora")}>
                          {t.exploreDebates}
                        </button>
                      </div>
                    </li>
                  );
                })}
            </ul>
            <ul className="demo-roster__grid">
              {roster
                .filter((entry) => !(FEATURED_ROSTER_QIDS as readonly string[]).includes(entry.qid))
                .map((entry) => {
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

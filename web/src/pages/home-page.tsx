// web/src/pages/home-page.tsx
import { Link, useNavigate } from "react-router-dom";
import { Navbar } from "@/components/layout/navbar";
import { EntitySearchBox } from "@/components/search/entity-search-box";
import { usePersonPicker } from "@/hooks/use-person-picker";
import { useI18n } from "@/lib/i18n";
import type { SearchSuggestion } from "@/lib/schemas/entity";

export function HomePage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const { suggestions, setSearchQuery, searchLoading, selectPerson } = usePersonPicker();

  function onSelect(item: SearchSuggestion) {
    selectPerson(item);
    navigate("/explorer");
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
                <span className="home-about__pillar-label">{t.livingMap}</span>
                <span className="home-about__pillar-text">{t.livingMapDesc}</span>
              </li>
              <li>
                <span className="home-about__pillar-label">{t.agora}</span>
                <span className="home-about__pillar-text">{t.agoraHint}</span>
              </li>
              <li>
                <span className="home-about__pillar-label">{t.sources}</span>
                <span className="home-about__pillar-text">{t.homeAboutSources}</span>
              </li>
            </ul>
          </div>
        </section>
      </main>
    </div>
  );
}

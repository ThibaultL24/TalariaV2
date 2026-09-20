// web/src/pages/whitepaper-page.tsx
import { Link } from "react-router-dom";
import { Navbar } from "@/components/layout/navbar";
import { useI18n } from "@/lib/i18n";
import {
  whitepaperForLocale,
  type WhitepaperSection,
  type WhitepaperSubsection,
} from "@/lib/whitepaper-content";

function BulletList({
  items,
  style = "ol",
}: {
  items: string[];
  style?: "ol" | "ul";
}) {
  const Tag = style === "ul" ? "ul" : "ol";
  return (
    <Tag className="whitepaper__list">
      {items.map((item) => (
        <li key={item.slice(0, 48)}>{item}</li>
      ))}
    </Tag>
  );
}

function SubsectionBlock({ subsection }: { subsection: WhitepaperSubsection }) {
  return (
    <div className="whitepaper__subsection">
      <h3 className="whitepaper__subsection-title">{subsection.title}</h3>
      {subsection.paragraphs?.map((paragraph) => (
        <p key={paragraph.slice(0, 48)} className="whitepaper__p">
          {paragraph}
        </p>
      ))}
      {subsection.bullets ? (
        <BulletList items={subsection.bullets} style={subsection.bulletStyle ?? "ul"} />
      ) : null}
    </div>
  );
}

function SectionBody({ section }: { section: WhitepaperSection }) {
  return (
    <>
      {section.paragraphs?.map((paragraph) => (
        <p key={paragraph.slice(0, 48)} className="whitepaper__p">
          {paragraph}
        </p>
      ))}

      {section.table ? (
        <div className="whitepaper__table-wrap">
          <table className="whitepaper__table">
            <thead>
              <tr>
                {section.table.headers.map((header) => (
                  <th key={header}>{header}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {section.table.rows.map((row) => (
                <tr key={row.join("|")}>
                  {row.map((cell, index) => (
                    <td key={`${row[0]}-${index}`}>{cell}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}

      {section.subsections?.map((subsection) => (
        <SubsectionBlock key={subsection.title} subsection={subsection} />
      ))}

      {section.cards ? (
        <div className="whitepaper__cards">
          {section.cards.map((card) => (
            <article key={card.title} className="whitepaper__card">
              <h3 className="whitepaper__card-title">{card.title}</h3>
              <p className="whitepaper__card-body">{card.body}</p>
            </article>
          ))}
        </div>
      ) : null}

      {section.bullets ? (
        <BulletList items={section.bullets} style={section.bulletStyle ?? "ol"} />
      ) : null}

      {section.quote ? (
        <blockquote className="whitepaper__quote">{section.quote}</blockquote>
      ) : null}
    </>
  );
}

export function WhitepaperPage() {
  const { t, locale } = useI18n();
  const doc = whitepaperForLocale(locale);

  return (
    <div className="app-shell whitepaper-shell">
      <Navbar />
      <main className="whitepaper">
        <header className="whitepaper__hero">
          <div className="whitepaper__hero-veil" aria-hidden />
          <div className="whitepaper__hero-inner">
            <p className="whitepaper__meta">{doc.meta}</p>
            <h1 className="whitepaper__brand">{doc.title}</h1>
            <p className="whitepaper__lede">{doc.lede}</p>
            {doc.intro?.map((paragraph) => (
              <p key={paragraph.slice(0, 48)} className="whitepaper__intro">
                {paragraph}
              </p>
            ))}
            <nav className="whitepaper__toc" aria-label={t.whitepaperToc}>
              {doc.sections.map((section) => (
                <a key={section.id} href={`#${section.id}`} className="whitepaper__toc-link">
                  <span className="whitepaper__toc-num">{section.eyebrow}</span>
                  {section.title}
                </a>
              ))}
            </nav>
          </div>
        </header>

        <div className="whitepaper__body">
          {doc.sections.map((section) => (
            <section key={section.id} id={section.id} className="whitepaper__section">
              <header className="whitepaper__section-head">
                {section.eyebrow ? (
                  <span className="whitepaper__eyebrow">{section.eyebrow}</span>
                ) : null}
                <h2 className="whitepaper__section-title">{section.title}</h2>
              </header>
              <SectionBody section={section} />
            </section>
          ))}

          <footer className="whitepaper__closing">
            {doc.closingTitle ? (
              <h2 className="whitepaper__closing-title">{doc.closingTitle}</h2>
            ) : null}
            {doc.closingLines?.map((line) => (
              <p key={line} className="whitepaper__closing-line">
                {line}
              </p>
            ))}
            <p className="whitepaper__closing-text">{doc.closing}</p>
            <div className="whitepaper__closing-actions">
              <Link className="button button--primary" to="/">
                {t.home}
              </Link>
              <Link className="button button--ghost" to="/agora">
                {t.openAgora}
              </Link>
            </div>
          </footer>
        </div>
      </main>
    </div>
  );
}

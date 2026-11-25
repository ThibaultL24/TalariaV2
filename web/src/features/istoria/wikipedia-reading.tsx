// web/src/features/istoria/wikipedia-reading.tsx
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { Link } from "react-router-dom";

interface WikiSection {
  index: string;
  line: string;
  anchor: string;
  toclevel: number;
  kind: "life" | "posterity" | "apparatus";
}

interface WikiReading {
  title: string;
  url: string;
  sections: WikiSection[];
}

interface SectionBody {
  paragraphs: string[];
  image?: { src: string; caption: string };
}

const APPARATUS = [
  "reference", "references", "bibliographie", "bibliography", "voir aussi", "see also",
  "lien externe", "liens externes", "external link", "external links", "notes", "notes et references",
  "further reading", "articles connexes",
];
const POSTERITY = [
  "culture populaire", "popular culture", "posterite", "legacy", "historiographie",
  "historiography", "hommage", "hommages", "in fiction", "dans la fiction",
];

function fold(value: string): string {
  return value.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
}

function sectionKind(line: string): WikiSection["kind"] {
  const name = fold(line);
  if (APPARATUS.some((item) => name === item || name.startsWith(`${item} `))) return "apparatus";
  if (POSTERITY.some((item) => name.includes(item))) return "posterity";
  return "life";
}

async function wikipediaTitle(qid: string, lang: "fr" | "en", signal: AbortSignal): Promise<string> {
  const site = lang === "fr" ? "frwiki" : "enwiki";
  const response = await fetch(
    `https://www.wikidata.org/w/api.php?action=wbgetentities&ids=${encodeURIComponent(qid)}&props=sitelinks&sitefilter=${site}&format=json&origin=*`,
    { signal },
  );
  if (!response.ok) throw new Error("wikidata");
  const body = await response.json() as {
    entities?: Record<string, { sitelinks?: Record<string, { title?: string }> }>;
  };
  const title = body.entities?.[qid]?.sitelinks?.[site]?.title;
  if (!title) throw new Error("no sitelink");
  return title;
}

async function wikipediaOutline(qid: string, lang: "fr" | "en", signal: AbortSignal): Promise<WikiReading> {
  const title = await wikipediaTitle(qid, lang, signal);
  const host = lang === "fr" ? "fr.wikipedia.org" : "en.wikipedia.org";
  const response = await fetch(
    `https://${host}/w/api.php?action=parse&page=${encodeURIComponent(title)}&prop=sections&format=json&origin=*&redirects=1`,
    { signal },
  );
  if (!response.ok) throw new Error("wikipedia");
  const body = await response.json() as {
    parse?: { title?: string; sections?: Array<{ index: string; line: string; anchor: string; toclevel: number }> };
  };
  const sections = (body.parse?.sections ?? []).map((section) => ({
    index: section.index,
    line: section.line.replace(/<[^>]+>/g, ""),
    anchor: section.anchor,
    toclevel: section.toclevel,
    kind: sectionKind(section.line),
  }));
  const page = encodeURIComponent((body.parse?.title ?? title).replace(/ /g, "_"));
  return { title: body.parse?.title ?? title, url: `https://${host}/wiki/${page}`, sections };
}

function bodyFromHtml(html: string): SectionBody {
  const document = new DOMParser().parseFromString(html, "text/html");
  document.querySelectorAll("sup, style, button, .mw-editsection, .noprint").forEach((node) => node.remove());
  const image = [...document.querySelectorAll("img")].find((node) => {
    const src = node.getAttribute("src") ?? "";
    return src.includes("upload.wikimedia.org") && (node.width === 0 || node.width >= 80);
  });
  const src = image?.getAttribute("src") ?? "";
  const paragraphs = [...document.querySelectorAll("p")]
    .map((node) => node.textContent?.replace(/\s+/g, " ").trim() ?? "")
    .filter((paragraph) => paragraph.length > 40 && !/redirige ici|redirects here/i.test(paragraph));
  return {
    paragraphs,
    image: image && src
      ? {
          src: src.startsWith("//") ? `https:${src}` : src,
          caption: image.getAttribute("alt") || image.closest("figure")?.querySelector("figcaption")?.textContent?.trim() || "",
        }
      : undefined,
  };
}

async function wikipediaSection(title: string, lang: "fr" | "en", index: string, signal: AbortSignal): Promise<SectionBody> {
  const host = lang === "fr" ? "fr.wikipedia.org" : "en.wikipedia.org";
  const response = await fetch(
    `https://${host}/w/api.php?action=parse&page=${encodeURIComponent(title)}&prop=text&section=${encodeURIComponent(index)}&format=json&origin=*&redirects=1`,
    { signal },
  );
  if (!response.ok) throw new Error("section");
  const body = await response.json() as { parse?: { text?: Record<string, string> } };
  const html = body.parse?.text?.["*"] ?? "";
  return bodyFromHtml(html);
}

export function WikipediaReading({
  qid,
  locale,
  entityId,
}: {
  qid: string;
  locale: "fr" | "en";
  entityId: string;
}) {
  const fr = locale === "fr";
  const [selected, setSelected] = useState("0");
  const outline = useQuery({
    queryKey: ["wikipedia-outline", qid, locale],
    queryFn: ({ signal }) => wikipediaOutline(qid, locale, signal),
    staleTime: 10 * 60_000,
    retry: false,
  });
  const section = useQuery({
    queryKey: ["wikipedia-section", outline.data?.title, locale, selected],
    enabled: Boolean(outline.data?.title),
    queryFn: ({ signal }) => wikipediaSection(outline.data!.title, locale, selected, signal),
    staleTime: 10 * 60_000,
    retry: false,
  });
  const narrative = (outline.data?.sections ?? []).filter((item) => item.kind !== "apparatus");
  const apparatus = (outline.data?.sections ?? []).filter((item) => item.kind === "apparatus");
  const current = narrative.find((item) => item.index === selected);

  return (
    <section className="wiki-reading" aria-label={fr ? "Biographie" : "Biography"}>
      <header className="wiki-reading__intro">
        <p>{fr ? "Wikipédia · lecture actuelle" : "Wikipedia · current reading"}</p>
        <h2>{outline.data?.title ?? (fr ? "Biographie" : "Biography")}</h2>
        <p>
          {fr
            ? "Les chapitres suivent l’article Wikipédia. Ils sont distincts des faits déjà admis dans le corpus Talaria."
            : "The chapters follow the Wikipedia article. They are separate from the facts already admitted in the Talaria corpus."}
        </p>
        <p className="wiki-reading__jumps">
          <Link to={`/istoria/${entityId}/map`}>{fr ? "Ouvrir le grand atlas" : "Open the atlas"}</Link>
          <Link to={`/istoria/${entityId}/timeline`}>{fr ? "Ouvrir la frise" : "Open the timeline"}</Link>
          {outline.data ? <a href={outline.data.url} target="_blank" rel="noreferrer">{fr ? "Article source" : "Source article"}</a> : null}
        </p>
      </header>
      {outline.isError ? (
        <p role="status">{fr ? "L’article Wikipédia n’est pas disponible pour le moment." : "The Wikipedia article is not available right now."}</p>
      ) : null}
      <div className="wiki-reading__layout">
        <nav aria-label={fr ? "Chapitres de l’article" : "Article chapters"}>
          <button type="button" aria-current={selected === "0" ? "true" : undefined} onClick={() => setSelected("0")}>
            {fr ? "Introduction" : "Introduction"}
          </button>
          {narrative.map((item) => (
            <button
              key={item.index}
              type="button"
              className={item.toclevel > 1 ? "is-nested" : undefined}
              aria-current={selected === item.index ? "true" : undefined}
              onClick={() => setSelected(item.index)}
            >
              {item.line}
              {item.kind === "posterity" ? <small>{fr ? "Postérité" : "Afterlife"}</small> : null}
            </button>
          ))}
          {apparatus.length > 0 ? (
            <details>
              <summary>{fr ? "Notes et références" : "Notes and references"}</summary>
              {apparatus.map((item) => (
                <button key={item.index} type="button" onClick={() => setSelected(item.index)}>{item.line}</button>
              ))}
            </details>
          ) : null}
        </nav>
        <article>
          {section.data?.image ? (
            <figure>
              <img src={section.data.image.src} alt={section.data.image.caption || (outline.data?.title ?? "")} />
              {section.data.image.caption ? <figcaption>{section.data.image.caption}</figcaption> : null}
            </figure>
          ) : null}
          <h3>{selected === "0" ? (fr ? "Introduction" : "Introduction") : current?.line}</h3>
          {section.isPending ? <p role="status">{fr ? "Lecture du chapitre…" : "Reading the chapter…"}</p> : null}
          {section.data?.paragraphs.map((paragraph) => <p key={paragraph.slice(0, 80)}>{paragraph}</p>)}
          {section.data && section.data.paragraphs.length === 0 ? (
            <p>{fr ? "Ce chapitre ne contient pas de paragraphe narratif." : "This chapter has no narrative paragraph."}</p>
          ) : null}
        </article>
      </div>
    </section>
  );
}

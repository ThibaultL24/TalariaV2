// web/src/features/istoria/chapter-reading.tsx
import { Link } from "react-router-dom";
import { eventDate, groupChapterEvents, type ChapterCoverage, type ChapterEvent, type EditorialGroupSummary } from "@/lib/entity-views";
import { useI18n } from "@/lib/i18n";

export function ChapterReading({
  group,
  chapterIndex,
  chapterCount,
  events,
  complete,
  loadingMore = false,
  onLoadMore,
  coverage,
  timelineTo,
  mapTo,
  onSelect,
}: {
  group: EditorialGroupSummary;
  chapterIndex: number;
  chapterCount: number;
  events: ChapterEvent[];
  complete: boolean;
  loadingMore?: boolean;
  onLoadMore?: () => void;
  coverage?: ChapterCoverage;
  timelineTo: string;
  mapTo: string;
  onSelect: (id: string) => void;
}) {
  const { t, locale } = useI18n();
  const grouped = groupChapterEvents(events);
  const places = events.filter((event) => event.coordinates);
  return (
    <article className="chapter-reading">
      <header>
        {chapterCount > 0 && (
          <p className="chapter-reading-kicker">{t.chapterPosition(chapterIndex, chapterCount)}</p>
        )}
        <h2>{group.title}</h2>
      </header>
      <section>
        <h3>{t.chapterIntroduction}</h3>
        {group.summary ? <p className="chapter-reading-intro">{group.summary}</p> : null}
        {group.summary_origin && (
          <p className="v3-note">{t.chapterProvenance} · {group.summary_origin}</p>
        )}
      </section>
      <RoleList variant="primary" title={t.chapterPrimary} events={grouped.primary} onSelect={onSelect} evidenceLabel={t.chapterEvidenceCount} />
      <RoleList variant="supporting" title={t.chapterSupporting} events={grouped.supporting} onSelect={onSelect} evidenceLabel={t.chapterEvidenceCount} />
      <RoleList variant="context" title={t.chapterContextFacts} events={grouped.context} onSelect={onSelect} evidenceLabel={t.chapterEvidenceCount} />
      {!events.length && <p>{t.chapterEmpty}</p>}
      {!complete && (
        <p className="chapter-reading-links">
          <button type="button" className="v3-load-more" disabled={loadingMore} onClick={onLoadMore}>
            {t.chapterLoadMore}
          </button>
        </p>
      )}
      <p className="chapter-reading-links">
        <Link to={timelineTo}>{t.openChapterTimeline}</Link>
        {places.length > 0 && <Link to={mapTo}>{t.openChapterMap}</Link>}
      </p>
      <section>
        <h3>{t.chapterPlaces}</h3>
        {places.length === 0 ? <p>{t.chapterNoPlaces}</p> : (
          <ul>
            {places.map((event) => (
              <li key={event.id}>
                <button type="button" onClick={() => onSelect(event.id)}>{event.place_label || event.title}</button>
                <span>{placePrecision(event, locale)}</span>
              </li>
            ))}
          </ul>
        )}
      </section>
      {coverage && (
        <section>
          <h3>{t.chapterLimits}</h3>
          <ul className="chapter-reading-coverage">
            <li>{t.chapterEstablished(coverage.established)}</li>
            <li>{t.chapterAttested(coverage.attested)}</li>
            <li>{t.chapterInReview(coverage.in_review)}</li>
            <li>{t.chapterWithoutPlace(coverage.without_coordinates)}</li>
            <li>{t.chapterImpreciseTime(coverage.time_not_exact)}</li>
            <li>{t.chapterWithEvidence(coverage.with_evidence)}</li>
            <li>{t.chapterPassages(coverage.evidence_rows)}</li>
            <li>{t.chapterIndependentSources(coverage.independent_sources)}</li>
          </ul>
          <p className="v3-note">{t.chapterDocumentsRepeat}</p>
          <p className="v3-note">{t.chapterNoCompleteness}</p>
        </section>
      )}
    </article>
  );
}

function RoleList({
  variant,
  title,
  events,
  onSelect,
  evidenceLabel,
}: {
  variant: "primary" | "supporting" | "context";
  title: string;
  events: ChapterEvent[];
  onSelect: (id: string) => void;
  evidenceLabel: (count: number) => string;
}) {
  const { t, locale } = useI18n();
  if (!events.length) return null;
  const items = events.map((event) => (
    <li key={event.id}>
      <button type="button" onClick={() => onSelect(event.id)}>
        <strong>{event.title}</strong>
        <span>{recordedDate(event, locale, t.chapterRecordedYear)}</span>
        {event.place_label && <span>{event.place_label}</span>}
        <span>{event.epistemic_status}</span>
        <span>{evidenceLabel(event.evidence_count)}</span>
      </button>
    </li>
  ));
  if (variant === "context") {
    return (
      <details className="chapter-reading-context">
        <summary>{title}</summary>
        <ul>{items}</ul>
      </details>
    );
  }
  const Tag = variant === "primary" ? "ol" : "ul";
  return (
    <section className={`chapter-reading-${variant}`}>
      <h3>{title}</h3>
      <Tag>{items}</Tag>
    </section>
  );
}

function recordedDate(
  event: ChapterEvent,
  locale: "fr" | "en",
  recordedYear: (year: string) => string,
): string {
  const year = event.time?.precision === "year" ? recordedYearValue(event.time.start, event.time.surface) : null;
  if (year) return recordedYear(year);
  return eventDate({ time: event.time } as Parameters<typeof eventDate>[0], locale);
}

function recordedYearValue(start?: string | null, surface?: string | null): string | null {
  const match = `${start ?? ""} ${surface ?? ""}`.match(/\b\d{3,4}\b/);
  return match?.[0] ?? null;
}

function placePrecision(event: ChapterEvent, locale: "fr" | "en"): string {
  const level = event.coord_precision_level;
  const radius = event.uncertainty_radius_m;
  if (level === "city" && radius != null) {
    const kilometers = Math.round(radius / 1000);
    return locale === "fr"
      ? `Référence de la localité, à ${kilometers} km près`
      : `Town reference, within ${kilometers} km`;
  }
  if (level) return level;
  return locale === "fr" ? "Lieu situé" : "Located place";
}

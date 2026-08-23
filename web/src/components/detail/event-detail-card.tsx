// web/src/components/detail/event-detail-card.tsx
import { useEffect, useRef, useState } from "react";
import { formatDateLabel } from "@/lib/geo";
import {
  fetchEventDetail,
  type EventDetailResponse,
  type EventSourceRef,
  type TimelineEvent,
} from "@/lib/api";
import { SourceRefsList } from "@/components/detail/source-refs-list";
import { EventImageHero } from "@/components/detail/event-image-hero";
import { CitedParagraph } from "@/components/detail/cited-paragraph";
import { resolveEventImage, type ResolvedEventImage } from "@/lib/resolve-event-image";
import { useI18n } from "@/lib/i18n";

interface EventDetailCardProps {
  event: TimelineEvent;
  onClose: () => void;
  offlineOnly?: boolean;
}

export function EventDetailCard({ event, onClose, offlineOnly = false }: EventDetailCardProps) {
  const { t } = useI18n();
  const [detail, setDetail] = useState<EventDetailResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeCite, setActiveCite] = useState<number | null>(null);
  const [image, setImage] = useState<ResolvedEventImage | null>(null);
  const [imageLoading, setImageLoading] = useState(false);
  const [sourcesOpen, setSourcesOpen] = useState(false);
  const sourcesRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setActiveCite(null);
    setSourcesOpen(false);
    fetchEventDetail(event.id)
      .then((payload) => {
        if (!cancelled) setDetail(payload);
      })
      .catch(() => {
        if (!cancelled) setDetail(null);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [event.id]);

  useEffect(() => {
    if (!detail?.event || offlineOnly) {
      setImage(null);
      setImageLoading(false);
      return;
    }
    let cancelled = false;
    setImageLoading(true);
    const wikiLang =
      detail.source_refs?.find((ref) => ref.language)?.language ??
      detail.evidence?.find((item) => item.wiki_lang)?.wiki_lang ??
      "en";
    resolveEventImage({
      eventType: detail.event.event_type,
      personLabel: detail.event.person,
      placeLabel: detail.event.place_label,
      sourcePageTitles: detail.source_page_titles,
      sourceRefs: detail.source_refs,
      wikipediaUrl: detail.links?.wikipedia_url,
      defaultLang: wikiLang,
    })
      .then((resolved) => {
        if (!cancelled) setImage(resolved);
      })
      .catch(() => {
        if (!cancelled) setImage(null);
      })
      .finally(() => {
        if (!cancelled) setImageLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [detail, offlineOnly]);

  const resolved = detail?.event ?? event;
  const summary =
    detail?.narrative?.event_summary?.trim() ||
    detail?.narrative?.how_it_happened?.trim() ||
    detail?.narrative?.fact?.trim() ||
    resolved.summary?.trim() ||
    null;
  const sourceRefs = collectSourceRefs(detail);
  const wikiLang =
    sourceRefs.find((ref) => ref.language)?.language ??
    detail?.evidence?.find((item) => item.wiki_lang)?.wiki_lang ??
    "en";

  function focusCitation(index: number) {
    setActiveCite(index);
    setSourcesOpen(true);
    window.setTimeout(() => {
      document.getElementById(`source-ref-${index}`)?.scrollIntoView({
        behavior: "smooth",
        block: "nearest",
      });
      sourcesRef.current?.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }, 50);
  }

  const datePlace = [formatDateLabel(resolved.start_time), resolved.place_label]
    .filter(Boolean)
    .join(" · ");

  return (
    <div className="flex h-full min-h-0 flex-col overflow-y-auto bg-(--color-bg-elevated)">
      <div className="flex items-start justify-between gap-3 border-b border-(--color-border-subtle) p-4">
        <div className="min-w-0">
          <h2 id="event-detail-card-title" className="text-lg font-semibold leading-snug">
            {resolved.title}
          </h2>
          {datePlace ? (
            <p className="mt-1 text-sm text-(--color-text-secondary)">{datePlace}</p>
          ) : null}
        </div>
        <button
          type="button"
          onClick={onClose}
          className="rounded p-1 text-xl leading-none hover:bg-(--color-bg-primary)"
          aria-label={t.closeDetail}
        >
          ×
        </button>
      </div>

      <div className="flex-1 space-y-4 p-4">
        {loading ? <p className="text-sm text-(--color-text-muted)">{t.loading}</p> : null}
        <EventImageHero image={image} loading={imageLoading} />
        {summary ? (
          <section>
            <h3 className="mb-2 text-[10px] font-semibold uppercase tracking-wide text-(--color-text-muted)">
              {t.summary}
            </h3>
            <CitedParagraph text={summary} onCiteClick={focusCitation} variant="lead" />
          </section>
        ) : null}

        <section ref={sourcesRef}>
          <button
            type="button"
            className="flex w-full items-center justify-between rounded-lg border border-(--color-border-subtle) px-3 py-2 text-left text-sm font-semibold"
            onClick={() => setSourcesOpen((open) => !open)}
            aria-expanded={sourcesOpen}
          >
            <span>
              {t.sources}
              {sourceRefs.length > 0 ? ` (${sourceRefs.length})` : ""}
            </span>
            <span className="text-(--color-text-muted)">{sourcesOpen ? "−" : "+"}</span>
          </button>
          {sourcesOpen ? (
            <div className="mt-2">
              <SourceRefsList
                refs={sourceRefs}
                wikiLang={wikiLang ?? undefined}
                activeCitationIndex={activeCite}
              />
            </div>
          ) : null}
        </section>
      </div>
    </div>
  );
}

function collectSourceRefs(detail: EventDetailResponse | null): EventSourceRef[] {
  if (!detail) return [];
  if (detail.source_refs && detail.source_refs.length > 0) return detail.source_refs;
  return (detail.evidence ?? []).map((item, index) => ({
    source_system: "wikipedia",
    language: item.wiki_lang,
    page_title: item.wiki_title,
    source_page_title: item.wiki_title,
    oldid: item.revision_id,
    revision_id: item.revision_id,
    snippet: item.quoted_text ?? item.sentence_text,
    quote: item.quoted_text ?? item.sentence_text,
    label: item.wiki_title ? `Wikipedia — ${item.wiki_title}` : "Wikipedia",
    url: item.citation_url ?? item.revision_url ?? item.page_url,
    source_url: item.citation_url ?? item.revision_url ?? item.page_url,
    wikipedia_url: item.page_url,
    page_url: item.page_url,
    revision_url: item.revision_url,
    confidence: item.confidence,
    evidence_id: item.id,
    citation_index: index + 1,
  }));
}

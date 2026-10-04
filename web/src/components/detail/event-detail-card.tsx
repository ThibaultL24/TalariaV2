// web/src/components/detail/event-detail-card.tsx
import { useEffect, useState } from "react";
import { formatDateLabel } from "@/lib/geo";
import {
  fetchEventDetail,
  type EventDetailResponse,
  type EventSourceRef,
  type TimelineEvent,
} from "@/lib/api";
import { resolveSourceParagraphHref } from "@/components/detail/source-ref-url";
import { shortLifeRecap } from "@/components/detail/how-it-happened";
import { eventDate } from "@/lib/entity-views";
import { useI18n } from "@/lib/i18n";
import { localizedEventTitle, localizedPlaceLabel } from "@/lib/localize-event-copy";

interface EventDetailCardProps {
  event: TimelineEvent;
  onClose: () => void;
  offlineOnly?: boolean;
}

export function EventDetailCard({ event, onClose, offlineOnly: _offlineOnly = false }: EventDetailCardProps) {
  const { t, locale } = useI18n();
  const [detail, setDetail] = useState<EventDetailResponse | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    fetchEventDetail(event.id, locale)
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
  }, [event.id, locale]);

  const resolved = detail?.event ?? event;
  const recap = shortLifeRecap(detail?.narrative?.event_summary ?? resolved.summary);
  const displayTitle = localizedEventTitle(resolved, locale);
  const mappedPlace = localizedPlaceLabel(resolved.place_label, locale);
  const placeLabel = mappedPlace && !/^Q\d+$/i.test(mappedPlace) ? mappedPlace : null;
  const sourceRefs = collectSourceRefs(detail);
  const wikiLang =
    sourceRefs.find((ref) => ref.language)?.language ??
    detail?.evidence?.find((item) => item.wiki_lang)?.wiki_lang ??
    "en";
  const passage = sourceRefs
    .map((ref) => resolveSourceParagraphHref(ref, wikiLang))
    .find((href): href is string => Boolean(href));
  const dateLabel = resolved.time ? eventDate(resolved) : formatDateLabel(resolved.start_time);
  const datePlace = [dateLabel, placeLabel].filter(Boolean).join(" · ");

  return (
    <div className="flex h-full min-h-0 flex-col overflow-y-auto bg-(--color-bg-elevated)">
      <div className="flex items-start justify-between gap-3 border-b border-(--color-border-subtle) p-4">
        <div className="min-w-0">
          <h2 id="event-detail-card-title" className="text-lg font-semibold leading-snug">
            {displayTitle}
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
        {recap ? <p className="text-sm leading-relaxed text-(--color-text-primary)">{recap}</p> : null}
        {passage ? (
          <a
            href={passage}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-block text-sm font-medium text-(--color-accent-strong) hover:underline"
          >
            {t.readPassage}
          </a>
        ) : null}
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

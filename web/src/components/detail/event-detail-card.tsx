// web/src/components/detail/event-detail-card.tsx
import { useEffect, useRef, useState } from "react";
import { formatDateLabel } from "@/lib/geo";
import {
  epistemicBadgeClass,
  epistemicStatusLabel,
  eventTypeLabel,
} from "@/lib/event-taxonomy";
import {
  fetchEntityClaims,
  fetchEventDetail,
  type EntityClaim,
  type EventDetailResponse,
  type EventSourceRef,
  type TimelineEvent,
} from "@/lib/api";
import { SourceRefsList } from "@/components/detail/source-refs-list";
import { HowItHappened } from "@/components/detail/how-it-happened";
import { EventImageHero } from "@/components/detail/event-image-hero";
import { AgoraPanel } from "@/components/explorer/agora-panel";
import {
  resolveEventImage,
  type ResolvedEventImage,
} from "@/lib/resolve-event-image";

interface EventDetailCardProps {
  event: TimelineEvent;
  onClose: () => void;
  offlineOnly?: boolean;
}

export function EventDetailCard({ event, onClose, offlineOnly = false }: EventDetailCardProps) {
  const [detail, setDetail] = useState<EventDetailResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [relatedDebates, setRelatedDebates] = useState<EntityClaim[]>([]);
  const [activeCite, setActiveCite] = useState<number | null>(null);
  const [image, setImage] = useState<ResolvedEventImage | null>(null);
  const [imageLoading, setImageLoading] = useState(false);
  const sourcesRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setActiveCite(null);
    setRelatedDebates([]);

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

    fetchEntityClaims(event.entity_id)
      .then((items) => {
        if (!cancelled) {
          setRelatedDebates(
            items.filter((claim) => claim.canonical_event_id === event.id),
          );
        }
      })
      .catch(() => {
        if (!cancelled) setRelatedDebates([]);
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
  const howItHappened =
    detail?.narrative?.how_it_happened?.trim() ||
    detail?.narrative?.event_summary?.trim() ||
    detail?.narrative?.fact?.trim() ||
    resolved.summary?.trim() ||
    null;
  const sourceRefs = resolveSourceRefs(detail);
  const wikiLang =
    sourceRefs.find((ref) => ref.language)?.language ??
    detail?.evidence?.find((item) => item.wiki_lang)?.wiki_lang ??
    "en";

  function focusCitation(index: number) {
    setActiveCite(index);
    const node = document.getElementById(`source-ref-${index}`);
    node?.scrollIntoView({ behavior: "smooth", block: "nearest" });
    sourcesRef.current?.scrollIntoView({ behavior: "smooth", block: "nearest" });
  }

  return (
    <div className="flex h-full min-h-0 flex-col overflow-y-auto bg-(--color-bg-elevated)">
      <div className="flex items-center justify-between border-b border-(--color-border-subtle) p-4">
        <h2 id="event-detail-card-title" className="text-base font-semibold">
          Event details
        </h2>
        <button
          type="button"
          onClick={onClose}
          className="rounded p-1 hover:bg-(--color-bg-primary)"
          aria-label="Close"
        >
          ×
        </button>
      </div>

      <div className="flex-1 space-y-4 p-4">
        <header>
          <div className="flex flex-wrap items-center gap-2">
            <p className="text-[10px] font-semibold uppercase tracking-wide text-(--color-text-secondary)">
              {eventTypeLabel(resolved.event_type)}
            </p>
            <span
              className={`inline-flex rounded-full px-2 py-0.5 text-[10px] font-medium ${epistemicBadgeClass(resolved.epistemic_status)}`}
            >
              {epistemicStatusLabel(resolved.epistemic_status)}
            </span>
          </div>
          <h3 className="mt-1 text-lg font-semibold leading-snug text-(--color-text-primary)">
            {resolved.title}
          </h3>
        </header>

        {loading ? (
          <p className="text-sm text-(--color-text-muted)">Loading sources…</p>
        ) : null}

        <EventImageHero image={image} loading={imageLoading} />

        {howItHappened ? (
          <HowItHappened text={howItHappened} onCiteClick={focusCitation} />
        ) : null}

        <dl className="grid gap-2 text-sm">
          <div>
            <dt className="text-(--color-text-secondary)">Person</dt>
            <dd className="font-medium text-(--color-text-primary)">{resolved.person}</dd>
          </div>
          <div>
            <dt className="text-(--color-text-secondary)">Date</dt>
            <dd className="font-medium text-(--color-text-primary)">
              {formatDateLabel(resolved.start_time)}
            </dd>
          </div>
          {resolved.place_label ? (
            <div>
              <dt className="text-(--color-text-secondary)">Place</dt>
              <dd className="font-medium text-(--color-text-primary)">{resolved.place_label}</dd>
            </div>
          ) : null}
          <div>
            <dt className="text-(--color-text-secondary)">Veracity</dt>
            <dd className="font-medium text-(--color-text-primary)">
              {epistemicStatusLabel(resolved.epistemic_status)}
            </dd>
          </div>
          <div>
            <dt className="text-(--color-text-secondary)">Model confidence</dt>
            <dd className="font-medium tabular-nums text-(--color-text-primary)">
              {Math.round(resolved.confidence * 100)}%
            </dd>
          </div>
        </dl>

        {(detail?.links?.wikipedia_url ||
          detail?.links?.wikipedia_revision_url ||
          detail?.links?.wikidata_url) && (
          <section className="border-t border-(--color-border-subtle) pt-4">
            <h4 className="mb-2 text-sm font-semibold text-(--color-text-primary)">Useful links</h4>
            <ul className="space-y-1 text-sm">
              {detail.links.wikipedia_revision_url ? (
                <li>
                  <a
                    href={detail.links.wikipedia_revision_url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-(--color-accent-strong) hover:underline"
                  >
                    Wikipedia revision
                  </a>
                </li>
              ) : null}
              {detail.links.wikipedia_url ? (
                <li>
                  <a
                    href={detail.links.wikipedia_url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-(--color-accent-strong) hover:underline"
                  >
                    Wikipedia article
                  </a>
                </li>
              ) : null}
              {detail.links.wikidata_url ? (
                <li>
                  <a
                    href={detail.links.wikidata_url}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-(--color-accent-strong) hover:underline"
                  >
                    Wikidata
                  </a>
                </li>
              ) : null}
            </ul>
          </section>
        )}

        {relatedDebates.length > 0 ? (
          <section className="border-t border-(--color-border-subtle) pt-4">
            <h4 className="mb-2 text-sm font-semibold text-(--color-text-primary)">
              Agora — linked debates
            </h4>
            <p className="mb-2 text-xs text-(--color-text-secondary)">
              Historiographic interpretations for this event — not quality map facts.
            </p>
            <AgoraPanel claims={relatedDebates} claimsOnly />
          </section>
        ) : null}

        <section ref={sourcesRef} className="border-t border-(--color-border-subtle) pt-4">
          <h4 className="mb-2 text-sm font-semibold text-(--color-text-primary)">
            Verifiable sources
          </h4>
          <p className="mb-3 text-xs leading-relaxed text-(--color-text-secondary)">
            Each [n] in the dossier points here. Prefer revision links (oldid) for stable citations.
          </p>
          <SourceRefsList
            refs={sourceRefs}
            wikiLang={wikiLang ?? undefined}
            activeCitationIndex={activeCite}
          />
        </section>
      </div>
    </div>
  );
}

function resolveSourceRefs(detail: EventDetailResponse | null): EventSourceRef[] {
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
    section_title:
      item.sentence_ordinal != null ? `sentence ${item.sentence_ordinal}` : null,
    sentence_ordinal: item.sentence_ordinal,
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

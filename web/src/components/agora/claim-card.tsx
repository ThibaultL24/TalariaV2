// web/src/components/agora/claim-card.tsx
import { useEffect, useState } from "react";
import {
  fetchClaimIntuition,
  type BibliographyItem,
  type EntityClaim,
  type InteractionSummary,
} from "@/lib/api";
import { ClaimActionBar } from "@/components/agora/claim-action-bar";
import { ClaimDiscussion } from "@/components/agora/claim-discussion";
import { ClaimGraph } from "@/components/agora/claim-graph";
import {
  debateTypeLabel,
  evidenceLayerLabel,
  isStanceClaimKind,
} from "@/lib/agora-taxonomy";
import { epistemicBadgeClass, epistemicStatusLabel } from "@/lib/event-taxonomy";
import { sourceKindBadgeClass, sourceSystemLabel } from "@/lib/source-labels";
import { useI18n } from "@/lib/i18n";

interface ClaimCardProps {
  claim: EntityClaim;
  onOpenEvent?: (eventId: string) => void;
  summary?: InteractionSummary;
  onChanged?: () => void;
  bibliography?: BibliographyItem[];
}

function evidenceHref(locator: string | null | undefined): string | null {
  if (!locator) return null;
  return /^https?:\/\//i.test(locator) ? locator : null;
}

export function ClaimCard({
  claim,
  onOpenEvent,
  summary,
  onChanged,
  bibliography,
}: ClaimCardProps) {
  const { locale, t } = useI18n();
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [intuitionLabel, setIntuitionLabel] = useState<string | null>(null);
  const debateType =
    debateTypeLabel(claim.debate_type, locale) ?? debateTypeLabel(claim.claim_kind, locale);
  const layer = evidenceLayerLabel(claim.evidence_layer, locale);
  const linked = claim.canonical_event_id;
  const stance = isStanceClaimKind(claim.claim_kind);

  useEffect(() => {
    if (!detailsOpen || !stance) return;
    let cancelled = false;
    fetchClaimIntuition(claim.id)
      .then((status) => {
        if (cancelled) return;
        setIntuitionLabel(
          status.status === "ready"
            ? `${t.intuitionPublished} · ${t.intuitionSignalAvailable}`
            : t.intuitionNotPublished,
        );
      })
      .catch(() => {
        if (!cancelled) setIntuitionLabel(null);
      });
    return () => {
      cancelled = true;
    };
  }, [detailsOpen, stance, claim.id, t.intuitionPublished, t.intuitionSignalAvailable, t.intuitionNotPublished]);

  return (
    <article className="nebula-timeline-card claim-card w-full p-3 text-left">
      <header className="claim-card__header">
        <div className="flex flex-wrap items-center gap-1.5">
          {debateType ? (
            <span className="text-[10px] font-semibold uppercase tracking-wide text-(--color-text-secondary)">
              {debateType}
            </span>
          ) : null}
          <span
            className={`inline-flex rounded-full px-2 py-0.5 text-[10px] font-medium ${epistemicBadgeClass(claim.epistemic_status)}`}
          >
            {epistemicStatusLabel(claim.epistemic_status, locale)}
          </span>
          {layer ? <span className="agora-layer-badge">{layer}</span> : null}
        </div>
        <p className="mt-2 text-sm leading-snug text-(--color-text-primary)">{claim.text}</p>
      </header>
      {stance ? (
        <p className="claim-card__counts">
          {t.sourcesCount(summary?.source_count ?? 0)} · {t.argumentsCount(summary?.argument_count ?? 0)}{" "}
          · {t.comments(summary?.comment_count ?? 0)}
        </p>
      ) : null}
      {linked && onOpenEvent ? (
        <button
          type="button"
          className="mt-2 text-[11px] text-(--color-accent-strong) hover:underline"
          onClick={() => onOpenEvent(linked)}
        >
          {t.relatedEvent}
        </button>
      ) : null}
      {stance ? (
        <>
          <ClaimActionBar
            claimId={claim.id}
            claimText={claim.text}
            summary={summary}
            intuitionStatusLabel={detailsOpen ? intuitionLabel : null}
            onChanged={onChanged}
          />
          <button
            type="button"
            className="claim-discussion__toggle"
            onClick={() => setDetailsOpen((open) => !open)}
          >
            {detailsOpen ? t.closeClaimDetails : t.openClaimDetails}
          </button>
          {detailsOpen ? (
            <div className="claim-card__details">
              <section className="claim-evidence">
                <h3>{t.corpusEvidenceTitle}</h3>
                {claim.evidence.length > 0 ? (
                  <ul className="mt-2 space-y-2 text-[11px] text-(--color-text-secondary)">
                    {claim.evidence.map((row) => {
                      const href =
                        evidenceHref(row.document_url) ?? evidenceHref(row.locator) ?? null;
                      const source = row.source_kind ?? row.source_system;
                      return (
                        <li key={row.id} className="agora-evidence-row">
                          <div className="flex flex-wrap items-center gap-1.5">
                            <span
                              className={`inline-flex rounded px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide ${sourceKindBadgeClass(source)}`}
                            >
                              {sourceSystemLabel(source, t.sourceFallback)}
                            </span>
                            {row.document_title ? (
                              <span className="text-(--color-text-primary)">{row.document_title}</span>
                            ) : null}
                          </div>
                          {href ? (
                            <a
                              href={href}
                              target="_blank"
                              rel="noopener noreferrer"
                              className="mt-1 inline-block text-(--color-accent-strong) hover:underline"
                            >
                              {t.openSource}
                            </a>
                          ) : null}
                          {row.quote ? (
                            <p className="mt-1 line-clamp-4 italic text-(--color-text-muted)">
                              {row.quote}
                            </p>
                          ) : null}
                        </li>
                      );
                    })}
                  </ul>
                ) : (
                  <p className="claim-section-empty">{t.noEvidenceLocator}</p>
                )}
              </section>
              <ClaimGraph
                claimId={claim.id}
                argumentCount={summary?.argument_count ?? 0}
                sourceCount={summary?.source_count ?? 0}
                bibliography={bibliography}
                forceOpen
                onCountChanged={onChanged}
              />
              <ClaimDiscussion
                claimId={claim.id}
                commentCount={summary?.comment_count ?? 0}
                bibliography={bibliography}
                forceOpen
                onCountChanged={onChanged}
              />
            </div>
          ) : null}
        </>
      ) : null}
    </article>
  );
}

// web/src/components/explorer/agora-panel.tsx
import { useEffect, useState } from "react";
import {
  fetchTheorySignals,
  simulateTheoryStance,
  type BibliographyItem,
  type EntityClaim,
  type TheoryStanceResponse,
} from "@/lib/api";
import {
  debateTypeLabel,
  evidenceLayerLabel,
  groupClaimsByDebateType,
} from "@/lib/agora-taxonomy";
import { epistemicBadgeClass, epistemicStatusLabel } from "@/lib/event-taxonomy";
import { sourceKindBadgeClass, sourceSystemLabel } from "@/lib/source-labels";
import { useI18n } from "@/lib/i18n";

interface AgoraPanelProps {
  claims: EntityClaim[];
  bibliography?: BibliographyItem[];
  isLoading?: boolean;
  bibliographyLoading?: boolean;
  onOpenEvent?: (eventId: string) => void;
  /** Hide bibliography block (e.g. event detail card). */
  claimsOnly?: boolean;
}

function evidenceHref(locator: string | null | undefined): string | null {
  if (!locator) return null;
  return /^https?:\/\//i.test(locator) ? locator : null;
}

function TheoryStanceBar({ claimId }: { claimId: string }) {
  const { t } = useI18n();
  const [signals, setSignals] = useState<TheoryStanceResponse | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    fetchTheorySignals(claimId)
      .then((payload) => {
        if (!cancelled) setSignals(payload);
      })
      .catch(() => {
        if (!cancelled) setSignals(null);
      });
    return () => {
      cancelled = true;
    };
  }, [claimId]);

  async function onStance(stance: "believe" | "dispute") {
    setBusy(true);
    setMessage(null);
    try {
      const result = await simulateTheoryStance(claimId, stance);
      if (result.code === "not_on_chain") setMessage(t.stanceNotOnChain);
      else if (result.code === "live_disabled") setMessage(t.stanceLiveDisabled);
      else setMessage(result.reason ?? t.stanceLiveDisabled);
    } catch {
      setMessage(t.stanceLiveDisabled);
    } finally {
      setBusy(false);
    }
  }

  const hint =
    message ??
    (signals?.code === "not_on_chain"
      ? t.stanceNotOnChain
      : signals?.code === "live_disabled"
        ? t.stanceLiveDisabled
        : t.stanceHint);

  return (
    <div className="mt-3 border-t border-(--color-border-subtle)/70 pt-2">
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          disabled={busy}
          onClick={() => void onStance("believe")}
          className="rounded-md border border-emerald-500/40 bg-emerald-500/10 px-2.5 py-1 text-[11px] font-medium text-emerald-100 disabled:opacity-40"
        >
          {t.believe}
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={() => void onStance("dispute")}
          className="rounded-md border border-rose-500/40 bg-rose-500/10 px-2.5 py-1 text-[11px] font-medium text-rose-100 disabled:opacity-40"
        >
          {t.dispute}
        </button>
      </div>
      <p className="mt-1.5 text-[10px] leading-relaxed text-(--color-text-muted)">{hint}</p>
    </div>
  );
}

function ClaimCard({
  claim,
  onOpenEvent,
}: {
  claim: EntityClaim;
  onOpenEvent?: (eventId: string) => void;
}) {
  const { locale, t } = useI18n();
  const debateType = debateTypeLabel(claim.debate_type, locale) ?? debateTypeLabel(claim.claim_kind, locale);
  const layer = evidenceLayerLabel(claim.evidence_layer, locale);
  const linked = claim.canonical_event_id;

  return (
    <article className="nebula-timeline-card w-full p-3 text-left">
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
        {layer ? (
          <span className="inline-flex rounded-full bg-white/10 px-2 py-0.5 text-[10px] text-(--color-text-muted)">
            {layer}
          </span>
        ) : null}
      </div>
      <p className="mt-2 text-sm leading-snug text-(--color-text-primary)">{claim.text}</p>
      {claim.evidence.length > 0 ? (
        <ul className="mt-2 space-y-2 text-[11px] text-(--color-text-secondary)">
          {claim.evidence.map((row) => {
            const href =
              evidenceHref(row.document_url) ??
              evidenceHref(row.locator) ??
              null;
            const source = row.source_kind ?? row.source_system;
            return (
              <li key={row.id} className="rounded-md border border-(--color-border-subtle)/60 bg-black/10 px-2 py-1.5">
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
                  <p className="mt-1 line-clamp-4 italic text-(--color-text-muted)">{row.quote}</p>
                ) : null}
              </li>
            );
          })}
        </ul>
      ) : (
        <p className="mt-2 text-[11px] text-(--color-text-muted)">{t.noEvidenceLocator}</p>
      )}
      {linked && onOpenEvent ? (
        <button
          type="button"
          className="mt-2 text-[11px] text-(--color-accent-strong) hover:underline"
          onClick={() => onOpenEvent(linked)}
        >
          {t.relatedEvent}
        </button>
      ) : null}
      {claim.claim_kind.trim().toLowerCase() === "theory" ? (
        <TheoryStanceBar claimId={claim.id} />
      ) : null}
    </article>
  );
}

function BibliographyList({ items }: { items: BibliographyItem[] }) {
  const { t } = useI18n();
  if (items.length === 0) return null;

  const bySource = new Map<string, BibliographyItem[]>();
  for (const item of items) {
    const key = item.source_kind ?? "other";
    const list = bySource.get(key) ?? [];
    list.push(item);
    bySource.set(key, list);
  }

  return (
    <div className="space-y-3">
      {[...bySource.entries()]
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([sourceKind, docs]) => (
          <div key={sourceKind}>
            <p className="mb-1.5 text-[10px] font-semibold uppercase tracking-wide text-(--color-text-muted)">
              {sourceSystemLabel(sourceKind, t.sourceFallback)}
            </p>
            <ul className="space-y-1.5">
              {docs.map((doc) => (
                <li
                  key={doc.id}
                  className="rounded-md border border-(--color-border-subtle)/60 bg-black/10 px-2.5 py-2"
                >
                  <p className="text-sm leading-snug text-(--color-text-primary)">{doc.title}</p>
                  <div className="mt-1 flex flex-wrap gap-2 text-[10px] text-(--color-text-muted)">
                    {doc.document_type ? <span>{doc.document_type.replace(/_/g, " ")}</span> : null}
                    {doc.language ? <span>{doc.language}</span> : null}
                    {doc.link?.score != null ? (
                      <span>{t.matchScore(Math.round(doc.link.score * 100))}</span>
                    ) : null}
                  </div>
                  {doc.canonical_url ? (
                    <a
                      href={doc.canonical_url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="mt-1 inline-block text-[11px] text-(--color-accent-strong) hover:underline"
                    >
                      {t.openDocument}
                    </a>
                  ) : null}
                </li>
              ))}
            </ul>
          </div>
        ))}
    </div>
  );
}

export function AgoraPanel({
  claims,
  bibliography = [],
  isLoading,
  bibliographyLoading,
  onOpenEvent,
  claimsOnly = false,
}: AgoraPanelProps) {
  const { t, locale } = useI18n();
  if (isLoading && claims.length === 0 && (claimsOnly || bibliography.length === 0)) {
    return <p className="p-4 text-center text-sm text-(--color-text-muted)">{t.loadingAgora}</p>;
  }

  const grouped = groupClaimsByDebateType(claims, locale, t.otherDebates);
  const empty = claims.length === 0 && bibliography.length === 0;

  if (empty && !isLoading && !bibliographyLoading) {
    return (
      <div className="space-y-3 p-4 text-center text-sm text-(--color-text-muted)">
        <p>{t.agoraEmpty}</p>
      </div>
    );
  }

  return (
    <div className="space-y-4 overflow-y-auto p-3">
      {!claimsOnly ? (
        <div className="rounded-lg border border-amber-500/25 bg-amber-500/10 px-3 py-2 text-[11px] leading-relaxed text-amber-100/90">
          <strong className="font-semibold">{t.laneAgoraTitle}</strong> — {t.laneAgoraHint}
        </div>
      ) : null}

      {claims.length > 0 ? (
        <section>
          {!claimsOnly ? (
            <h3 className="mb-2 text-[10px] font-semibold uppercase tracking-wide text-(--color-text-secondary)">
              {t.laneAgoraTitle}
            </h3>
          ) : null}
          <div className="space-y-4">
            {grouped.map((group) => (
              <div key={group.key}>
                {grouped.length > 1 ? (
                  <p className="mb-1.5 text-[10px] font-medium uppercase tracking-wide text-(--color-text-muted)">
                    {group.label} ({group.claims.length})
                  </p>
                ) : null}
                <div className="space-y-2">
                  {group.claims.map((claim) => (
                    <ClaimCard key={claim.id} claim={claim} onOpenEvent={onOpenEvent} />
                  ))}
                </div>
              </div>
            ))}
          </div>
        </section>
      ) : null}

      {!claimsOnly && (bibliographyLoading || bibliography.length > 0) ? (
        <section className="border-t border-(--color-border-subtle) pt-3">
          <h3 className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-(--color-text-secondary)">
            {t.bibliographyTitle}
          </h3>
          <p className="mb-2 text-[10px] leading-relaxed text-(--color-text-muted)">
            {t.bibliographyHint}
          </p>
          {bibliographyLoading && bibliography.length === 0 ? (
            <p className="text-xs text-(--color-text-muted)">{t.loadingSources}</p>
          ) : (
            <BibliographyList items={bibliography} />
          )}
        </section>
      ) : null}
    </div>
  );
}

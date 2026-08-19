// web/src/components/explorer/agora-panel.tsx
import type { BibliographyItem, EntityClaim } from "@/lib/api";
import {
  debateTypeLabel,
  evidenceLayerLabel,
  groupClaimsByDebateType,
} from "@/lib/agora-taxonomy";
import { epistemicBadgeClass, epistemicStatusLabel } from "@/lib/event-taxonomy";
import { sourceKindBadgeClass, sourceSystemLabel } from "@/lib/source-labels";
import { strings } from "@/lib/strings";

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

function ClaimCard({
  claim,
  onOpenEvent,
}: {
  claim: EntityClaim;
  onOpenEvent?: (eventId: string) => void;
}) {
  const debateType = debateTypeLabel(claim.debate_type) ?? debateTypeLabel(claim.claim_kind);
  const layer = evidenceLayerLabel(claim.evidence_layer);
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
          {epistemicStatusLabel(claim.epistemic_status)}
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
                    {sourceSystemLabel(source)}
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
                    Open source
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
        <p className="mt-2 text-[11px] text-(--color-text-muted)">No evidence locator.</p>
      )}
      {linked && onOpenEvent ? (
        <button
          type="button"
          className="mt-2 text-[11px] text-(--color-accent-strong) hover:underline"
          onClick={() => onOpenEvent(linked)}
        >
          Related quality event
        </button>
      ) : null}
    </article>
  );
}

function BibliographyList({ items }: { items: BibliographyItem[] }) {
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
              {sourceSystemLabel(sourceKind)}
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
                      <span>match {Math.round(doc.link.score * 100)}%</span>
                    ) : null}
                  </div>
                  {doc.canonical_url ? (
                    <a
                      href={doc.canonical_url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="mt-1 inline-block text-[11px] text-(--color-accent-strong) hover:underline"
                    >
                      Open document
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
  if (isLoading && claims.length === 0 && (claimsOnly || bibliography.length === 0)) {
    return <p className="p-4 text-center text-sm text-(--color-text-muted)">Loading agora…</p>;
  }

  const grouped = groupClaimsByDebateType(claims);
  const empty = claims.length === 0 && bibliography.length === 0;

  if (empty && !isLoading && !bibliographyLoading) {
    return (
      <div className="space-y-3 p-4 text-center text-sm text-(--color-text-muted)">
        <p>{strings.emptyAgora}</p>
      </div>
    );
  }

  return (
    <div className="space-y-4 overflow-y-auto p-3">
      {!claimsOnly ? (
        <div className="rounded-lg border border-amber-500/25 bg-amber-500/10 px-3 py-2 text-[11px] leading-relaxed text-amber-100/90">
          <strong className="font-semibold">{strings.laneAgoraTitle}</strong> —{" "}
          {strings.laneAgoraHint}
        </div>
      ) : null}

      {claims.length > 0 ? (
        <section>
          {!claimsOnly ? (
            <h3 className="mb-2 text-[10px] font-semibold uppercase tracking-wide text-(--color-text-secondary)">
              Debates & interpretations
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
            Linked bibliography
          </h3>
          <p className="mb-2 text-[10px] leading-relaxed text-(--color-text-muted)">
            Academic catalogs linked to this subject — metadata only, not map events.
          </p>
          {bibliographyLoading && bibliography.length === 0 ? (
            <p className="text-xs text-(--color-text-muted)">Loading sources…</p>
          ) : (
            <BibliographyList items={bibliography} />
          )}
        </section>
      ) : null}
    </div>
  );
}

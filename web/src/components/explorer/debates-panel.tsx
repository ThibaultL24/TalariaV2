// web/src/components/explorer/debates-panel.tsx
import type { EntityClaim } from "@/lib/api";
import { epistemicBadgeClass, epistemicStatusLabel } from "@/lib/event-taxonomy";

interface DebatesPanelProps {
  claims: EntityClaim[];
  isLoading?: boolean;
  onOpenEvent?: (eventId: string) => void;
}

function prettySlug(value: string | null | undefined): string | null {
  if (!value) return null;
  return value.replace(/_/g, " ");
}

function evidenceHref(locator: string | null | undefined): string | null {
  if (!locator) return null;
  return /^https?:\/\//i.test(locator) ? locator : null;
}

export function DebatesPanel({ claims, isLoading, onOpenEvent }: DebatesPanelProps) {
  if (isLoading && claims.length === 0) {
    return <p className="p-4 text-center text-sm text-(--color-text-muted)">Loading debates…</p>;
  }

  if (claims.length === 0) {
    return (
      <div className="p-4 text-center text-sm text-(--color-text-muted)">
        No sourced debates for this person yet. Ingest OpenAlex or run historiography-extract.
      </div>
    );
  }

  return (
    <div className="space-y-2 overflow-y-auto p-3">
      {claims.map((claim) => {
        const debateType = prettySlug(claim.debate_type) ?? prettySlug(claim.claim_kind);
        const layer = prettySlug(claim.evidence_layer);
        const linked = claim.canonical_event_id;

        return (
          <article key={claim.id} className="nebula-timeline-card w-full p-3 text-left">
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
              <ul className="mt-2 space-y-1 text-[11px] text-(--color-text-secondary)">
                {claim.evidence.map((row) => {
                  const href = evidenceHref(row.locator);
                  return (
                    <li key={row.id}>
                      <span className="uppercase">{row.source_system}</span>
                      {href ? (
                        <>
                          {" · "}
                          <a
                            href={href}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="text-(--color-accent-strong) hover:underline"
                          >
                            source
                          </a>
                        </>
                      ) : null}
                      {row.quote ? (
                        <p className="mt-0.5 line-clamp-3 italic text-(--color-text-muted)">
                          {row.quote}
                        </p>
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
                Related event
              </button>
            ) : null}
          </article>
        );
      })}
    </div>
  );
}

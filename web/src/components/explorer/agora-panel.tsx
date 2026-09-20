// web/src/components/explorer/agora-panel.tsx
import { useEffect, useMemo, useState } from "react";
import {
  type BibliographyItem,
  type EntityClaim,
} from "@/lib/api";
import {
  IntuitionStanceBar,
  isStanceClaimKind,
} from "@/components/intuition/intuition-stance-bar";
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

type AgoraTab = "theories" | "debates" | "bibliography";

function evidenceHref(locator: string | null | undefined): string | null {
  if (!locator) return null;
  return /^https?:\/\//i.test(locator) ? locator : null;
}

function claimSources(claim: EntityClaim): string[] {
  const keys = new Set<string>();
  for (const row of claim.evidence) {
    const raw = (row.source_kind ?? row.source_system ?? "").trim().toLowerCase();
    if (raw) keys.add(raw);
  }
  return [...keys];
}

function claimMatchesSource(claim: EntityClaim, source: string | null): boolean {
  if (!source) return true;
  const sources = claimSources(claim);
  if (source === "other") return sources.length === 0;
  return sources.includes(source);
}

function ClaimCard({
  claim,
  onOpenEvent,
}: {
  claim: EntityClaim;
  onOpenEvent?: (eventId: string) => void;
}) {
  const { locale, t } = useI18n();
  const debateType =
    debateTypeLabel(claim.debate_type, locale) ?? debateTypeLabel(claim.claim_kind, locale);
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
            const href = evidenceHref(row.document_url) ?? evidenceHref(row.locator) ?? null;
            const source = row.source_kind ?? row.source_system;
            return (
              <li
                key={row.id}
                className="rounded-md border border-(--color-border-subtle)/60 bg-black/10 px-2 py-1.5"
              >
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
      {isStanceClaimKind(claim.claim_kind) ? (
        <IntuitionStanceBar targetKind="claim" targetId={claim.id} />
      ) : null}
    </article>
  );
}

function SourceFilterBar({
  sources,
  counts,
  active,
  onChange,
}: {
  sources: string[];
  counts: Map<string, number>;
  active: string | null;
  onChange: (source: string | null) => void;
}) {
  const { t } = useI18n();
  if (sources.length === 0) return null;

  const total = [...counts.values()].reduce((a, b) => a + b, 0);

  return (
    <div
      className="flex flex-wrap gap-1.5"
      role="toolbar"
      aria-label={t.agoraSourceFilter}
    >
      <button
        type="button"
        onClick={() => onChange(null)}
        className={`rounded-md px-2.5 py-1 text-[11px] font-medium transition-colors ${
          active == null
            ? "bg-white/15 text-(--color-text-primary) ring-1 ring-white/25"
            : "bg-black/20 text-(--color-text-muted) hover:bg-white/10 hover:text-(--color-text-secondary)"
        }`}
      >
        {t.showAll}
        <span className="ml-1 tabular-nums opacity-70">{total}</span>
      </button>
      {sources.map((source) => (
        <button
          type="button"
          key={source}
          onClick={() => onChange(active === source ? null : source)}
          className={`rounded-md px-2.5 py-1 text-[11px] font-medium transition-colors ${
            active === source
              ? `${sourceKindBadgeClass(source)} ring-1 ring-white/30`
              : "bg-black/20 text-(--color-text-muted) hover:bg-white/10 hover:text-(--color-text-secondary)"
          }`}
        >
          {sourceSystemLabel(source, t.sourceFallback)}
          <span className="ml-1 tabular-nums opacity-70">{counts.get(source) ?? 0}</span>
        </button>
      ))}
    </div>
  );
}

function BibliographyList({ items }: { items: BibliographyItem[] }) {
  const { t } = useI18n();
  if (items.length === 0) {
    return <p className="text-sm text-(--color-text-muted)">{t.agoraFilterEmpty}</p>;
  }

  return (
    <ul className="space-y-1.5">
      {items.map((doc) => (
        <li
          key={doc.id}
          className="rounded-md border border-(--color-border-subtle)/60 bg-black/10 px-2.5 py-2"
        >
          <div className="mb-1 flex flex-wrap items-center gap-1.5">
            <span
              className={`inline-flex rounded px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide ${sourceKindBadgeClass(doc.source_kind)}`}
            >
              {sourceSystemLabel(doc.source_kind, t.sourceFallback)}
            </span>
            {doc.document_type ? (
              <span className="text-[10px] text-(--color-text-muted)">
                {doc.document_type.replace(/_/g, " ")}
              </span>
            ) : null}
          </div>
          <p className="text-sm leading-snug text-(--color-text-primary)">{doc.title}</p>
          <div className="mt-1 flex flex-wrap gap-2 text-[10px] text-(--color-text-muted)">
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
          <IntuitionStanceBar
            targetKind="source"
            targetId={doc.id}
            compact
          />
        </li>
      ))}
    </ul>
  );
}

function ClaimList({
  claims,
  onOpenEvent,
  emptyLabel,
}: {
  claims: EntityClaim[];
  onOpenEvent?: (eventId: string) => void;
  emptyLabel: string;
}) {
  const { t, locale } = useI18n();
  if (claims.length === 0) {
    return <p className="text-sm text-(--color-text-muted)">{emptyLabel}</p>;
  }

  const grouped = groupClaimsByDebateType(claims, locale, t.otherDebates);
  return (
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
  );
}

function sourceCountsForClaims(claims: EntityClaim[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const claim of claims) {
    const seen = new Set(claimSources(claim));
    if (seen.size === 0) {
      counts.set("other", (counts.get("other") ?? 0) + 1);
      continue;
    }
    for (const source of seen) {
      counts.set(source, (counts.get(source) ?? 0) + 1);
    }
  }
  return counts;
}

function sourceCountsForBibliography(items: BibliographyItem[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const item of items) {
    const key = (item.source_kind ?? "other").trim().toLowerCase() || "other";
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return counts;
}

function sortedSources(counts: Map<string, number>): string[] {
  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([key]) => key);
}

export function AgoraPanel({
  claims,
  bibliography = [],
  isLoading,
  bibliographyLoading,
  onOpenEvent,
  claimsOnly = false,
}: AgoraPanelProps) {
  const { t } = useI18n();
  const [tab, setTab] = useState<AgoraTab>("theories");
  const [sourceFilter, setSourceFilter] = useState<string | null>(null);

  const theories = useMemo(
    () => claims.filter((c) => c.claim_kind.trim().toLowerCase() === "theory"),
    [claims],
  );
  const debates = useMemo(
    () => claims.filter((c) => c.claim_kind.trim().toLowerCase() !== "theory"),
    [claims],
  );

  useEffect(() => {
    setSourceFilter(null);
  }, [tab]);

  useEffect(() => {
    if (claimsOnly) return;
    if (tab === "theories" && theories.length === 0 && debates.length > 0) {
      setTab("debates");
    } else if (tab === "theories" && theories.length === 0 && bibliography.length > 0) {
      setTab("bibliography");
    } else if (tab === "debates" && debates.length === 0 && theories.length > 0) {
      setTab("theories");
    }
  }, [claimsOnly, tab, theories.length, debates.length, bibliography.length]);

  if (isLoading && claims.length === 0 && (claimsOnly || bibliography.length === 0)) {
    return <p className="p-4 text-center text-sm text-(--color-text-muted)">{t.loadingAgora}</p>;
  }

  const empty = claims.length === 0 && bibliography.length === 0;
  if (empty && !isLoading && !bibliographyLoading) {
    return (
      <div className="space-y-3 p-4 text-center text-sm text-(--color-text-muted)">
        <p>{t.agoraEmpty}</p>
      </div>
    );
  }

  const activeClaims = tab === "theories" ? theories : debates;
  const claimCounts = sourceCountsForClaims(activeClaims);
  const biblioCounts = sourceCountsForBibliography(bibliography);
  const filterSources =
    tab === "bibliography" ? sortedSources(biblioCounts) : sortedSources(claimCounts);
  const filterCounts = tab === "bibliography" ? biblioCounts : claimCounts;

  const filteredClaims = activeClaims.filter((c) => claimMatchesSource(c, sourceFilter));
  const filteredBiblio =
    sourceFilter == null
      ? bibliography
      : bibliography.filter(
          (item) => (item.source_kind ?? "other").trim().toLowerCase() === sourceFilter,
        );

  const tabs: Array<{ id: AgoraTab; label: string; count: number; hidden?: boolean }> = [
    { id: "theories", label: t.agoraTabTheories, count: theories.length },
    { id: "debates", label: t.agoraTabDebates, count: debates.length },
    {
      id: "bibliography",
      label: t.agoraTabBibliography,
      count: bibliography.length,
      hidden: claimsOnly,
    },
  ];

  return (
    <div className="space-y-3 overflow-y-auto p-3">
      {!claimsOnly ? (
        <div className="rounded-lg border border-amber-500/25 bg-amber-500/10 px-3 py-2 text-[11px] leading-relaxed text-amber-100/90">
          <strong className="font-semibold">{t.laneAgoraTitle}</strong> — {t.laneAgoraHint}
        </div>
      ) : null}

      <div
        className="flex flex-wrap gap-1 border-b border-(--color-border-subtle)/80 pb-2"
        role="tablist"
        aria-label={t.agora}
      >
        {tabs
          .filter((entry) => !entry.hidden)
          .map((entry) => {
            const selected = tab === entry.id;
            return (
              <button
                key={entry.id}
                type="button"
                role="tab"
                aria-selected={selected}
                onClick={() => setTab(entry.id)}
                className={`rounded-md px-3 py-1.5 text-[12px] font-medium transition-colors ${
                  selected
                    ? "bg-white/12 text-(--color-text-primary) ring-1 ring-white/20"
                    : "text-(--color-text-muted) hover:bg-white/5 hover:text-(--color-text-secondary)"
                }`}
              >
                {entry.label}
                <span className="ml-1.5 tabular-nums text-[10px] opacity-65">{entry.count}</span>
              </button>
            );
          })}
      </div>

      {tab === "theories" ? (
        <p className="text-[10px] leading-relaxed text-(--color-text-muted)">
          {t.intuitionTheoriesHint}
        </p>
      ) : null}
      {tab === "bibliography" ? (
        <p className="text-[10px] leading-relaxed text-(--color-text-muted)">{t.bibliographyHint}</p>
      ) : null}

      <SourceFilterBar
        sources={filterSources}
        counts={filterCounts}
        active={sourceFilter}
        onChange={setSourceFilter}
      />

      {tab === "bibliography" ? (
        bibliographyLoading && bibliography.length === 0 ? (
          <p className="text-xs text-(--color-text-muted)">{t.loadingSources}</p>
        ) : (
          <BibliographyList items={filteredBiblio} />
        )
      ) : isLoading && filteredClaims.length === 0 ? (
        <p className="text-xs text-(--color-text-muted)">{t.loadingAgora}</p>
      ) : (
        <ClaimList
          claims={filteredClaims}
          onOpenEvent={onOpenEvent}
          emptyLabel={t.agoraFilterEmpty}
        />
      )}
    </div>
  );
}

// web/src/components/agora/argument-card.tsx
import type { AgoraArgument } from "@/lib/api";
import { useI18n } from "@/lib/i18n";

export function ArgumentCard({ argument }: { argument: AgoraArgument }) {
  const { t } = useI18n();
  const relationLabel =
    argument.relation === "supports"
      ? t.argumentSupports
      : argument.relation === "contradicts"
        ? t.argumentContradicts
        : t.argumentQualifies;
  const author =
    argument.author?.display_name?.trim() ||
    (argument.origin === "user" ? t.anonymousHistorian : t.pipelineOrigin);
  const source = argument.sources[0];
  const quote = argument.evidence[0]?.quote;
  const isProposed = argument.origin === "user" || argument.contribution_status === "proposed";

  return (
    <article className="argument-card" data-relation={argument.relation} data-origin={argument.origin}>
      <p className="argument-card__meta">
        <span>{relationLabel}</span>
        <span>· {author}</span>
      </p>
      {isProposed ? (
        <p className="agora-community-badge">
          {t.communityContribution}
          {argument.contribution_status === "proposed" ? ` · ${t.proposedStatus}` : ""}
        </p>
      ) : null}
      <p className="argument-card__statement">{argument.statement}</p>
      {source ? (
        <p className="argument-card__source">
          {source.source_kind} · {source.title}
        </p>
      ) : null}
      {quote ? <p className="argument-card__evidence">{quote}</p> : null}
    </article>
  );
}

// web/src/components/agora/claim-graph.tsx
import { useEffect, useState } from "react";
import { AgoraModal } from "@/components/agora/agora-modal";
import { ArgumentCard } from "@/components/agora/argument-card";
import { ArgumentForm } from "@/components/agora/argument-form";
import {
  fetchClaimArguments,
  fetchClaimSources,
  postClaimArgument,
  postClaimSource,
  type AgoraArgument,
  type BibliographyItem,
  type ClaimSource,
} from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import { useTalariaSession } from "@/hooks/use-talaria-session";

interface ClaimGraphProps {
  claimId: string;
  argumentCount: number;
  sourceCount: number;
  bibliography?: BibliographyItem[];
  forceOpen?: boolean;
  onCountChanged?: () => void;
}

export function ClaimGraph({
  claimId,
  argumentCount,
  sourceCount,
  bibliography = [],
  forceOpen = false,
  onCountChanged,
}: ClaimGraphProps) {
  const { t } = useI18n();
  const session = useTalariaSession();
  const [open, setOpen] = useState(forceOpen);
  const [sources, setSources] = useState<ClaimSource[]>([]);
  const [argumentsList, setArgumentsList] = useState<AgoraArgument[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [sourceModal, setSourceModal] = useState(false);
  const [argumentModal, setArgumentModal] = useState(false);
  const [documentId, setDocumentId] = useState(bibliography[0]?.id ?? "");

  async function load() {
    setLoading(true);
    setError(null);
    try {
      const [src, args] = await Promise.all([
        fetchClaimSources(claimId),
        fetchClaimArguments(claimId),
      ]);
      setSources(src.items);
      setArgumentsList(args.items);
      setLoaded(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : t.sectionError);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    if (forceOpen) void load();
  }, [forceOpen, claimId]);

  async function toggle() {
    const next = !open;
    setOpen(next);
    if (next && !loaded) await load();
  }

  function requireSession(): boolean {
    if (session.authenticated) return true;
    setMessage(t.argumentSignInRequired);
    return false;
  }

  async function onAddSource() {
    if (!requireSession()) return;
    if (!documentId) {
      setMessage(t.sourceNotIndexed);
      return;
    }
    const { source } = await postClaimSource(claimId, documentId);
    setSources((prev) =>
      prev.some((row) => row.corpus_document_id === source.corpus_document_id)
        ? prev
        : [...prev, source],
    );
    setSourceModal(false);
    onCountChanged?.();
  }

  const supports = argumentsList.filter((item) => item.relation === "supports");
  const against = argumentsList.filter((item) => item.relation === "contradicts");
  const nuances = argumentsList.filter((item) => item.relation === "qualifies");

  const body = (
    <div className="claim-graph__body">
      {message ? <p role="status">{message}</p> : null}
      {loading ? <p role="status">{t.loading}</p> : null}
      {error ? (
        <p role="alert">
          {error}{" "}
          <button type="button" onClick={() => void load()}>
            {t.retrySection}
          </button>
        </p>
      ) : null}
      <section className="agora-source-section">
        <div className="claim-section-heading">
          <h3>{t.claimSourcesTitle}</h3>
          <button
            type="button"
            className="intuition-stance-btn"
            onClick={() => {
              if (!requireSession()) return;
              setSourceModal(true);
            }}
          >
            {t.addSource}
          </button>
        </div>
        {!loading && loaded && sources.length === 0 ? (
          <p className="claim-section-empty">{t.noSourcesYet}</p>
        ) : null}
        <ul className="agora-source-list">
          {sources.map((source) => (
            <li key={source.corpus_document_id} className="agora-source-row">
              <p className="agora-source-row__title">{source.title}</p>
              <p className="agora-source-row__meta">
                {source.source_kind}
                {source.document_type ? ` · ${source.document_type}` : ""}
              </p>
              {source.canonical_url ? (
                <a href={source.canonical_url} target="_blank" rel="noopener noreferrer">
                  {source.canonical_url}
                </a>
              ) : null}
              <p className="agora-community-badge">{t.communityContribution}</p>
            </li>
          ))}
        </ul>
      </section>
      <section className="agora-argument-section">
        <div className="claim-section-heading">
          <h3>{t.argumentsTitle}</h3>
          <button
            type="button"
            className="intuition-stance-btn"
            onClick={() => {
              if (!requireSession()) return;
              setArgumentModal(true);
            }}
          >
            {t.addArgument}
          </button>
        </div>
        {!loading && loaded && argumentsList.length === 0 ? (
          <p className="claim-section-empty">{t.noArgumentsYet}</p>
        ) : null}
        <h4>{t.argumentsFor}</h4>
        {supports.map((item) => (
          <ArgumentCard key={item.id} argument={item} />
        ))}
        <h4>{t.argumentsAgainst}</h4>
        {against.map((item) => (
          <ArgumentCard key={item.id} argument={item} />
        ))}
        <h4>{t.argumentsQualify}</h4>
        {nuances.map((item) => (
          <ArgumentCard key={item.id} argument={item} />
        ))}
      </section>
      <AgoraModal title={t.addSource} open={sourceModal} onClose={() => setSourceModal(false)}>
        {bibliography.length ? (
          <label>
            {t.selectSource}
            <select value={documentId} onChange={(event) => setDocumentId(event.target.value)}>
              {bibliography.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.title}
                </option>
              ))}
            </select>
            <button type="button" onClick={() => void onAddSource()}>
              {t.addSource}
            </button>
          </label>
        ) : (
          <p>{t.sourceNotIndexed}</p>
        )}
      </AgoraModal>
      <AgoraModal title={t.addArgument} open={argumentModal} onClose={() => setArgumentModal(false)}>
        {bibliography.length ? (
          <ArgumentForm
            bibliography={bibliography}
            submitLabel={t.submitArgument}
            onSubmit={async (payload) => {
              const { argument } = await postClaimArgument(claimId, payload);
              setArgumentsList((prev) => [argument, ...prev]);
              setArgumentModal(false);
              onCountChanged?.();
            }}
          />
        ) : (
          <p>{t.sourceNotIndexed}</p>
        )}
      </AgoraModal>
    </div>
  );

  return (
    <div className="claim-graph">
      {forceOpen ? null : (
        <button type="button" className="claim-discussion__toggle" onClick={() => void toggle()}>
          {open ? t.closeArguments : t.openArguments} · {t.argumentsCount(argumentCount)} ·{" "}
          {t.sourcesCount(sourceCount)}
        </button>
      )}
      {open ? body : null}
    </div>
  );
}

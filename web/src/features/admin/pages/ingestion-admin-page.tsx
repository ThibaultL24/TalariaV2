import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Navbar } from "@/components/layout/navbar";
import {
  AdminApiError, fetchAdminJobs, fetchAdminJobMetrics, fetchAdminSummary,
  type AdminIngestionSummary, type AdminJobMetrics, type AdminJobPage,
  type IngestProviderStats,
} from "@/lib/admin-ingest";

const formatCount = (value: number) => new Intl.NumberFormat("fr-FR").format(value);
const formatMs = (value: number | null) =>
  value == null ? "—" : value >= 1000 ? `${(value / 1000).toFixed(1)} s` : `${Math.round(value)} ms`;
const formatRate = (value: number | null) =>
  value == null ? "—" : `${value.toLocaleString("fr-FR", { maximumFractionDigits: 2 })} %`;
const formatDate = (value: string) =>
  new Intl.DateTimeFormat("fr-FR", { dateStyle: "short", timeStyle: "short" }).format(new Date(value));

function ErrorMessage({ error }: { error: Error | null }) {
  if (!error) return null;
  const auth = error instanceof AdminApiError && (error.status === 401 || error.status === 403);
  return <p className="ingestion-admin__error" role="alert">
    {error.message}
    {auth ? " L'accès doit être autorisé côté serveur ; la connexion seule ne suffit pas." : null}
  </p>;
}

function Stat({ name, value, caption }: { name: string; value: string; caption?: string }) {
  return <div className="ingestion-admin__stat">
    <span>{name}</span><strong>{value}</strong>{caption ? <small>{caption}</small> : null}
  </div>;
}

function ProviderTable({ providers }: { providers: IngestProviderStats[] }) {
  return <div className="ingestion-admin__table-wrap">
    <table className="ingestion-admin__table">
      <thead><tr>
        <th>Fournisseur</th><th>Tentatives</th><th>P95</th>
        <th>Fetchs</th><th>Timeouts</th><th>Erreurs / 429</th>
        <th>Documents liés</th><th>Taux de liaison</th>
      </tr></thead>
      <tbody>
        {providers.map((item) => <tr key={item.provider}>
          <th scope="row">{item.provider}</th>
          <td>{formatCount(item.attempts)}</td><td>{formatMs(item.p95_ms)}</td>
          <td>{formatCount(item.fetch_calls)}</td><td>{formatCount(item.timeouts)}</td>
          <td>{item.failed} / {item.rate_limited}</td>
          <td>{item.documents_matched_unique} / {item.documents_persisted_unique}</td>
          <td>
            <span>{formatRate(item.document_match_percent)}</span>
            <div className="ingestion-admin__meter" aria-hidden="true">
              <span style={{ width: `${item.document_match_percent ?? 0}%` }} />
            </div>
          </td>
        </tr>)}
      </tbody>
    </table>
    {providers.length === 0 ? <p className="ingestion-admin__empty">Aucune tentative documentée sur cette période.</p> : null}
  </div>;
}

function JobDetail({ id, onClose }: { id: string; onClose: () => void }) {
  const [data, setData] = useState<AdminJobMetrics | null>(null);
  const [error, setError] = useState<Error | null>(null);
  useEffect(() => {
    const controller = new AbortController();
    fetchAdminJobMetrics(id, controller.signal).then((result) => {
      if (!controller.signal.aborted) setData(result);
    }).catch((reason: unknown) => {
      if (!controller.signal.aborted) setError(reason instanceof Error ? reason : new Error(String(reason)));
    });
    return () => controller.abort();
  }, [id]);
  return <section className="ingestion-admin__panel ingestion-admin__detail" aria-label="Détails du job">
    <div className="ingestion-admin__section-heading">
      <div><h2>Diagnostic du job</h2><p><code>{id}</code></p></div>
      <button type="button" onClick={onClose}>Fermer</button>
    </div>
    <ErrorMessage error={error} />
    {!data && !error ? <p role="status">Chargement du diagnostic…</p> : null}
    {data ? <>
      <div className="ingestion-admin__kpis">
        <Stat name="Tentatives" value={String(data.attempt_count)} />
        <Stat name="Documents distincts" value={formatCount(data.coverage.documents_persisted_unique)} />
        <Stat name="Documents liés" value={formatCount(data.coverage.documents_matched_unique)} />
        <Stat name="Taux de liaison" value={formatRate(data.coverage.match_rate_percent)} />
      </div>
      <p className="ingestion-admin__note">Ces liens documentaires ne constituent pas des preuves historiques vérifiées.</p>
      <div className="ingestion-admin__table-wrap">
        <table className="ingestion-admin__table">
          <thead><tr><th>Source</th><th>Essai</th><th>État</th><th>Type d’erreur</th><th>Durée</th><th>Fetchs</th><th>Timeouts</th></tr></thead>
          <tbody>{data.provider_attempts.map((item) => <tr key={`${item.provider}-${item.attempt}`}>
            <th scope="row">{item.provider}</th><td>{item.attempt}</td>
            <td>{item.interrupted ? "Interrompu" : item.status}</td>
            <td>{item.error_kind ?? "—"}</td><td>{formatMs(item.elapsed_ms)}</td>
            <td>{item.fetch_calls}</td><td>{item.fetch_timeouts}</td>
          </tr>)}</tbody>
        </table>
      </div>
      {data.pagination.next_offset != null ? <p className="ingestion-admin__note">Historique limité aux 100 premières tentatives.</p> : null}
    </> : null}
  </section>;
}

export function IngestionAdminPage() {
  const [days, setDays] = useState(30);
  const [lane, setLane] = useState("");
  const [status, setStatus] = useState("");
  const [offset, setOffset] = useState(0);
  const [revision, setRevision] = useState(0);
  const [summary, setSummary] = useState<AdminIngestionSummary | null>(null);
  const [jobs, setJobs] = useState<AdminJobPage | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError(null);
    setSummary(null);
    setJobs(null);
    Promise.all([
      fetchAdminSummary(days, controller.signal),
      fetchAdminJobs(days, lane, status, offset, controller.signal),
    ]).then(([nextSummary, nextJobs]) => {
      if (controller.signal.aborted) return;
      setSummary(nextSummary);
      setJobs(nextJobs);
    }).catch((reason: unknown) => {
      if (controller.signal.aborted) return;
      setError(reason instanceof Error ? reason : new Error(String(reason)));
    }).finally(() => {
      if (!controller.signal.aborted) setLoading(false);
    });
    return () => controller.abort();
  }, [days, lane, status, offset, revision]);

  return <div className="ingestion-admin">
    <Navbar />
    <main className="ingestion-admin__main">
      <div className="ingestion-admin__breadcrumb"><Link to="/">Talaria</Link><span> / </span>Administration / Ingestions</div>
      <header className="ingestion-admin__hero">
        <div>
          <span className="ingestion-admin__eyebrow">ENGINE / OBSERVABILITY</span>
          <h1>Observatoire des ingestions</h1>
          <p>Surveillez les collectes documentaires d’Agora et les jobs biographiques d’Istoria.</p>
        </div>
        <button type="button" onClick={() => setRevision((value) => value + 1)}>Actualiser</button>
      </header>

      <section className="ingestion-admin__filters" aria-label="Filtres d'administration">
        <label>Période
          <select aria-label="Période" value={days} onChange={(e) => { setDays(Number(e.target.value)); setOffset(0); }}>
            <option value={7}>7 jours</option><option value={30}>30 jours</option><option value={90}>90 jours</option>
          </select>
        </label>
        <label>Expérience
          <select aria-label="Expérience" value={lane} onChange={(e) => { setLane(e.target.value); setOffset(0); }}>
            <option value="">Toutes</option><option value="explorer">Istoria</option><option value="agora">Agora</option>
          </select>
        </label>
        <label>État
          <select aria-label="État" value={status} onChange={(e) => { setStatus(e.target.value); setOffset(0); }}>
            <option value="">Tous</option><option value="queued">En attente</option><option value="running">En cours</option>
            <option value="done">Terminés</option><option value="failed">Échoués</option>
          </select>
        </label>
      </section>

      <ErrorMessage error={error} />
      {loading ? <p role="status" className="ingestion-admin__loading">Chargement des diagnostics…</p> : null}
      {summary ? <>
        <section className="ingestion-admin__kpis" aria-label="Indicateurs de jobs">
          <Stat name="Ingestions" value={formatCount(summary.jobs.total)} caption="Sur la période" />
          <Stat name="Terminées" value={formatCount(summary.jobs.done)} caption={`${summary.jobs.running} en cours`} />
          <Stat name="Échecs" value={formatCount(summary.jobs.failed)} caption={`${summary.jobs.queued} en attente`} />
          <Stat name="Relancées" value={formatCount(summary.jobs.retried)} caption="Plusieurs tentatives" />
          <Stat name="Durée P50" value={formatMs(summary.jobs.p50_ms)} />
          <Stat name="Durée P95" value={formatMs(summary.jobs.p95_ms)} />
        </section>
        <section className="ingestion-admin__panel">
          <div className="ingestion-admin__section-heading">
            <div><h2>Performance par fournisseur</h2>
              <p>Durées et incidents par tentative. Taux de liaison documentaire, et non de véracité historique.</p>
            </div>
          </div>
          <ProviderTable providers={summary.providers} />
        </section>
      </> : null}
      {jobs ? <section className="ingestion-admin__panel">
        <div className="ingestion-admin__section-heading"><div>
          <h2>Historique des jobs</h2><p>{formatCount(jobs.pagination.total)} résultat(s) sur la période sélectionnée</p>
        </div></div>
        <div className="ingestion-admin__table-wrap">
          <table className="ingestion-admin__table">
            <thead><tr><th>Personnalité</th><th>Expérience</th><th>État</th><th>Tentatives</th><th>Durée</th><th>Démarré</th><th>Diagnostic</th></tr></thead>
            <tbody>{jobs.items.map((job) => <tr key={job.job_id}>
              <th scope="row">{job.subject}<small>{job.qid ?? "Sans QID"}</small></th>
              <td>{job.lane === "agora" ? "Agora" : "Istoria"}</td><td>{job.status}</td>
              <td>{job.attempts}</td><td>{formatMs(job.duration_ms)}</td>
              <td>{formatDate(job.created_at)}</td>
              <td><button type="button" onClick={() => setSelectedId(job.job_id)}>Inspecter</button></td>
            </tr>)}</tbody>
          </table>
        </div>
        {jobs.items.length===0 ? <p className="ingestion-admin__empty">Aucun job pour ces filtres.</p> : null}
        <div className="ingestion-admin__pagination">
          <button type="button" disabled={offset===0} onClick={() => setOffset(Math.max(0,offset-25))}>Précédent</button>
          <span>{offset+1}–{Math.min(offset+25,jobs.pagination.total)} / {jobs.pagination.total}</span>
          <button type="button" disabled={jobs.pagination.next_offset===null} onClick={() => setOffset(jobs.pagination.next_offset ?? offset)}>Suivant</button>
        </div>
      </section> : null}
      {selectedId ? <JobDetail key={selectedId} id={selectedId} onClose={() => setSelectedId(null)} /> : null}
    </main>
  </div>;
}

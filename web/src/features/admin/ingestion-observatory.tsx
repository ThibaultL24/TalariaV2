import { useState, type FormEvent, type ReactNode } from "react";
import { Link, useParams } from "react-router-dom";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import {
  adminGet,
  AdminError,
  type Job,
  type Page,
  type Overview,
  type Provider,
  type Detail,
} from "./ingestion-api";
import "./ingestion-observatory.css";
const duration = (n: number | null) =>
  n === null
    ? "Non mesuré"
    : `${(n / 1000).toLocaleString("fr", { maximumFractionDigits: 1 })} s`;
const percent = (n: number | null) =>
  n === null ? "Non mesuré" : `${n.toFixed(1)} %`;
const date = (s: string | null) => (s ? new Date(s).toLocaleString("fr") : "—");
const denied = (e: unknown) =>
  e instanceof AdminError && [401, 403].includes(e.status);
function useAdmin<T>(path: string, active = true, terminal?: (data: T) => boolean) {
  return useQuery({
    queryKey: ["ingestion-admin", path],
    queryFn: ({ signal }) => adminGet<T>(path, signal),
    enabled: active,
    staleTime: 10_000,
    gcTime: 0,
    retry: false,
    placeholderData: keepPreviousData,
    refetchInterval: (query) =>
      denied(query.state.error) || (query.state.data && terminal?.(query.state.data)) ? false : 15_000,
    refetchIntervalInBackground: false,
  });
}
function Shell({ children }: { children: ReactNode }) {
  return (
    <main className="observatory">
      <header>
        <Link to="/">Talaria</Link>
        <p>Administration</p>
        <h1>Ingestion Observatory</h1>
        <p>Santé des collectes Istoria et Agora</p>
      </header>
      {children}
    </main>
  );
}
function Pager({
  offset,
  total,
  limit,
  onPage,
}: {
  offset: number;
  total: number;
  limit: number;
  onPage: (n: number) => void;
}) {
  return (
    <nav aria-label="Pagination">
      <button
        disabled={offset === 0}
        onClick={() => onPage(Math.max(0, offset - limit))}
      >
        Précédent
      </button>
      <span>
        {total === 0 ? 0 : offset + 1}–{Math.min(offset + limit, total)} /{" "}
        {total}
      </span>
      <button
        disabled={offset + limit >= total || offset + limit > 10000}
        onClick={() => onPage(offset + limit)}
      >
        Suivant
      </button>
    </nav>
  );
}
function Table({ children }: { children: ReactNode }) {
  return (
    <div className="observatory-table">
      <table>{children}</table>
    </div>
  );
}
function Metric({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="observatory-metric">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}
export function IngestionObservatory() {
  const [filters, setFilters] = useState(
    () =>
      new URLSearchParams({
        from: new Date(Date.now() - 7 * 86400000).toISOString(),
        until: new Date(new Date().setUTCHours(24, 0, 0, 0)).toISOString(),
      }),
  );
  const [offset, setOffset] = useState(0);
  const [providerOffset, setProviderOffset] = useState(0);
  const [sort, setSort] = useState("slowest");
  const overview = useAdmin<Overview>(`overview?${filters}`);
  const allowed = !!overview.data && !denied(overview.error);
  const jobs = useAdmin<Page<Job>>(
    `jobs?${filters}&limit=25&offset=${offset}`,
    allowed,
  );
  const providers = useAdmin<Page<Provider>>(
    `providers?${filters}&limit=25&offset=${providerOffset}&sort=${sort}`,
    allowed,
  );
  function apply(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const form = new FormData(e.currentTarget);
    const q = new URLSearchParams();
    for (const [key, value] of form)
      if (typeof value === "string" && value.trim())
        q.set(
          key,
          ["from", "until"].includes(key)
            ? new Date(`${value}Z`).toISOString()
            : value.trim(),
        );
    setFilters(q);
    setOffset(0);
    setProviderOffset(0);
  }
  const error = overview.error || jobs.error || providers.error;
  if (denied(error))
    return (
      <Shell>
        <p role="alert">{error?.message}</p>
        <Link to="/">Retour à Talaria pour se connecter</Link>
      </Shell>
    );
  return (
    <Shell>
      <form onSubmit={apply} className="observatory-filters">
        <label>
          Depuis (UTC)
          <input
            required
            aria-label="Depuis (UTC)"
            name="from"
            type="datetime-local"
            defaultValue={filters.get("from")!.slice(0, 16)}
          />
        </label>
        <label>
          Jusqu’à (UTC, exclu)
          <input
            required
            name="until"
            type="datetime-local"
            defaultValue={filters.get("until")!.slice(0, 16)}
          />
        </label>
        <label>
          Lane
          <select name="lane">
            <option value="">Toutes</option>
            <option value="explorer">Istoria</option>
            <option value="agora">Agora</option>
          </select>
        </label>
        <label>
          Statut
          <select name="status">
            <option value="">Tous</option>
            {["queued", "running", "done", "failed"].map((s) => (
              <option key={s}>{s}</option>
            ))}
          </select>
        </label>
        <label>
          QID
          <input name="qid" placeholder="Q517" pattern="Q[1-9][0-9]*" />
        </label>
        <label>
          Personnalité (nom exact)
          <input name="subject" maxLength={240} />
        </label>
        <label>
          Fournisseur
          <input name="provider" placeholder="hal" maxLength={64} />
        </label>
        <button type="submit">Appliquer les filtres</button>
      </form>
      <p>
        Période de création des jobs, maximum 93 jours. Actualisation toutes les
        15 secondes lorsque la page est visible.
      </p>
      <button
        onClick={() => {
          void overview.refetch();
          void jobs.refetch();
          void providers.refetch();
        }}
      >
        Actualiser
      </button>
      <span role="status">
        {overview.isFetching || jobs.isFetching || providers.isFetching
          ? " Actualisation…"
          : " Données à jour"}
      </span>
      {error && (
        <p role="alert">
          {error.message} Les dernières données disponibles restent affichées.
        </p>
      )}
      {overview.isPending && <p>Chargement du dashboard…</p>}
      {overview.data && (
        <>
          <section aria-label="Vue générale">
            <h2>Vue générale</h2>
            <dl className="observatory-metrics">
              {(["queued", "running", "done", "failed"] as const).map((s) => (
                <Metric key={s} label={s} value={overview.data.summary[s]} />
              ))}
              <Metric
                label="Durée moyenne"
                value={duration(overview.data.summary.mean_ms)}
              />
              <Metric
                label="p50"
                value={duration(overview.data.summary.p50_ms)}
              />
              <Metric
                label="p95"
                value={duration(overview.data.summary.p95_ms)}
              />
              <Metric
                label="Succès"
                value={percent(overview.data.summary.success_percent)}
              />
              <Metric
                label="Échec"
                value={percent(overview.data.summary.failure_percent)}
              />
              <Metric label="Reprises" value={overview.data.summary.resumes} />
              <Metric
                label="Leases actuellement expirés"
                value={overview.data.summary.expired_leases}
              />
              <Metric
                label="Documents distincts persistés"
                value={overview.data.summary.documents_persisted_unique}
              />
              <Metric
                label="Documents distincts reliés"
                value={overview.data.summary.documents_matched_unique}
              />
            </dl>
            <p>
              Durées : jobs terminés (done et failed), du premier démarrage à la
              fin, attentes de reprise incluses. Taux : jobs terminés
              uniquement.
            </p>
            <p>
              Couverture documentaire : journal Agora seulement (
              {overview.data.summary.instrumented_agora_jobs} jobs
              instrumentés). Les compteurs par fournisseur Istoria ne sont pas
              disponibles. Une liaison documentaire ne mesure pas la vérité
              historique.
            </p>
          </section>
          <section>
            <h2>Volume quotidien (UTC)</h2>
            {overview.data.volume.length === 0 ? (
              <p>Aucune ingestion sur cette période.</p>
            ) : (
              <ul className="observatory-volume">
                {overview.data.volume.map((v) => (
                  <li key={v.day}>
                    <span>{v.day}</span>
                    <meter
                      min={0}
                      max={Math.max(
                        1,
                        ...overview.data!.volume.map((x) => x.jobs),
                      )}
                      value={v.jobs}
                    >
                      {v.jobs}
                    </meter>
                    <strong>{v.jobs}</strong>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </>
      )}
      <section>
        <h2>Jobs</h2>
        {jobs.isPending && allowed && <p>Chargement des jobs…</p>}
        {jobs.data && (
          <>
            <Table>
              <thead>
                <tr>
                  {[
                    "Personnalité",
                    "Lane",
                    "Statut",
                    "Création",
                    "Durée",
                    "Tentatives",
                    "Reprises",
                    "Lease",
                  ].map((x) => (
                    <th key={x}>{x}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {jobs.data.items.map((j) => (
                  <tr key={j.id}>
                    <td>
                      <Link to={`/admin/ingestion/jobs/${j.id}`}>
                        {j.subject} · {j.qid ?? "QID non résolu"}
                      </Link>
                    </td>
                    <td>{j.lane === "explorer" ? "Istoria" : "Agora"}</td>
                    <td>{j.status}</td>
                    <td>{date(j.created_at)}</td>
                    <td>{duration(j.duration_ms)}</td>
                    <td>{j.attempts}</td>
                    <td>{j.resumes}</td>
                    <td>{j.lease_expired ? "Expiré" : "—"}</td>
                  </tr>
                ))}
              </tbody>
            </Table>
            {!jobs.data.items.length && <p>Aucun job correspondant.</p>}
            <Pager
              offset={offset}
              total={jobs.data.total}
              limit={25}
              onPage={setOffset}
            />
          </>
        )}
      </section>
      <section>
        <h2>Fournisseurs</h2>
        <label>
          Classement
          <select
            value={sort}
            onChange={(e) => {
              setSort(e.target.value);
              setProviderOffset(0);
            }}
          >
            <option value="slowest">Plus lents (p95)</option>
            <option value="calls">Plus d’appels</option>
            <option value="errors">Plus fort taux d’échec</option>
          </select>
        </label>
        <p>
          Durées : tentatives terminées. Échec : partial, failed ou rate_limited
          parmi les tentatives terminées configurées. Rate limits : tentatives
          classées, pas nombre exact de réponses HTTP 429.
        </p>
        {providers.data && (
          <>
            <Table>
              <thead>
                <tr>
                  {[
                    "Fournisseur",
                    "Tentatives",
                    "Moyenne / p50 / p95",
                    "Découverte / récupération",
                    "Découverts / persistés (opérations)",
                    "Distincts / reliés",
                    "Liaison",
                    "Timeouts / erreurs",
                    "Rate limits",
                    "Échec",
                    "Reprises / interruptions",
                  ].map((x) => (
                    <th key={x}>{x}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {providers.data.items.map((p) => (
                  <tr key={p.provider}>
                    <th>{p.provider}</th>
                    <td>{p.attempts}</td>
                    <td>
                      {duration(p.mean_ms)} / {duration(p.p50_ms)} /{" "}
                      {duration(p.p95_ms)}
                    </td>
                    <td>
                      {p.discover_calls} / {p.fetch_calls}
                    </td>
                    <td>
                      {p.discovered} / {p.persisted}
                    </td>
                    <td>
                      {p.documents_persisted_unique} /{" "}
                      {p.documents_matched_unique}
                    </td>
                    <td>{percent(p.match_rate_percent)}</td>
                    <td>
                      {p.timeouts} / {p.errors}
                    </td>
                    <td>{p.rate_limited_attempts}</td>
                    <td>{percent(p.failure_percent)}</td>
                    <td>
                      {p.resumes} / {p.interrupted_attempts}
                    </td>
                  </tr>
                ))}
              </tbody>
            </Table>
            {!providers.data.items.length && (
              <p>Aucune tentative fournisseur instrumentée.</p>
            )}
            <Pager
              offset={providerOffset}
              limit={25}
              total={providers.data.total}
              onPage={setProviderOffset}
            />
          </>
        )}
      </section>
    </Shell>
  );
}
export function IngestionJobDetail() {
  const { jobId } = useParams();
  const [offset, setOffset] = useState(0);
  const detail = useAdmin<Detail>(
    `jobs/${encodeURIComponent(jobId ?? "")}?limit=25&offset=${offset}`,
    true,
    (data) => data.job.status === "done" || data.job.status === "failed",
  );
  if (denied(detail.error))
    return (
      <Shell>
        <p role="alert">{detail.error?.message}</p>
      </Shell>
    );
  const d = detail.data;
  return (
    <Shell>
      <Link to="/admin/ingestion">← Observatoire</Link>
      <h2>Détail du job</h2>
      {detail.isPending && <p>Chargement du job…</p>}
      {detail.error && <p role="alert">{detail.error.message}</p>}
      {d && (
        <>
          <h3>
            {d.job.subject} · {d.job.qid ?? "QID non résolu"}
          </h3>
          <p>
            {d.job.id} · {d.job.lane} · {d.job.status}
          </p>
          <dl className="observatory-metrics">
            <Metric label="Création" value={date(d.job.created_at)} />
            <Metric label="Démarrage" value={date(d.job.started_at)} />
            <Metric label="Fin" value={date(d.job.finished_at)} />
            <Metric label="Durée" value={duration(d.job.duration_ms)} />
            <Metric label="Tentatives" value={d.job.attempts} />
            <Metric label="Reprises" value={d.job.resumes} />
            <Metric
              label="Lease expiré"
              value={d.job.lease_expired ? "Oui" : "Non"}
            />
            <Metric
              label="Documents distincts / reliés"
              value={`${d.metrics.coverage.documents_persisted_unique} / ${d.metrics.coverage.documents_matched_unique}`}
            />
          </dl>
          <p>
            Couverture documentaire Agora, sans validation de vérité historique.
            Les interruptions sont déduites des tentatives et de la lease
            actuelle ; aucun historique exhaustif de heartbeats n’est
            disponible.
          </p>
          <h3>Historique des tentatives, erreurs et interruptions</h3>
          <Table>
            <thead>
              <tr>
                {[
                  "Fournisseur",
                  "Tentative",
                  "Statut",
                  "Début / fin",
                  "Durée",
                  "Découverte / récupération",
                  "Découverts / persistés / ignorés",
                  "Liaisons (opérations)",
                  "Timeouts / erreurs",
                  "Catégorie",
                  "Interruption",
                ].map((x) => (
                  <th key={x}>{x}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {d.metrics.provider_attempts.map((a) => (
                <tr key={`${a.provider}-${a.attempt}`}>
                  <th>{a.provider}</th>
                  <td>{a.attempt}</td>
                  <td>{a.status}</td>
                  <td>
                    {date(a.started_at)} / {date(a.finished_at)}
                  </td>
                  <td>
                    {a.finished_at
                      ? duration(a.elapsed_ms)
                      : "En cours / incomplète"}
                  </td>
                  <td>
                    {a.discover_calls} / {a.fetch_calls}
                  </td>
                  <td>
                    {a.documents_discovered} / {a.documents_persisted} /{" "}
                    {a.documents_skipped}
                  </td>
                  <td>{a.match_operations}</td>
                  <td>
                    {a.fetch_timeouts} / {a.connector_errors}
                  </td>
                  <td>{a.error_kind ?? "—"}</td>
                  <td>{a.interrupted ? "Oui" : "Non"}</td>
                </tr>
              ))}
            </tbody>
          </Table>
          {!d.metrics.provider_attempts.length && (
            <p>Aucune tentative instrumentée sur cette page.</p>
          )}
          <h3>Checkpoints terminés</h3>
          <ul>
            {d.checkpoints.items.map((c) => (
              <li key={c.source_kind}>
                {c.source_kind} · {c.connector_version} · {date(c.completed_at)}
              </li>
            ))}
          </ul>
          {!d.checkpoints.items.length && (
            <p>Aucun checkpoint sur cette page.</p>
          )}
          <Pager
            offset={offset}
            total={Math.max(d.metrics.pagination.total, d.checkpoints.total)}
            limit={25}
            onPage={setOffset}
          />
        </>
      )}
    </Shell>
  );
}

// web/src/lib/person-ingest.ts
import {
  fetchIngestJob,
  startExplorerIngest,
  EXPLORER_INGEST_MAX_DOCUMENTS,
  type IngestJobResponse,
} from "@/lib/api";

const INGEST_POLL_MS = 1500;
const INGEST_TIMEOUT_MS = 45 * 60 * 1000;

let ingestLock: string | null = null;

export async function pollIngestJob(
  jobId: string,
  onTick: (job: IngestJobResponse) => void,
): Promise<IngestJobResponse> {
  const deadline = Date.now() + INGEST_TIMEOUT_MS;
  while (Date.now() < deadline) {
    const job = await fetchIngestJob(jobId);
    onTick(job);
    if (job.status !== "running" && job.status !== "queued") return job;
    await new Promise((resolve) => setTimeout(resolve, INGEST_POLL_MS));
  }
  throw new Error("timeout");
}

export async function runExplorerPersonIngest(input: {
  subject: string;
  qid?: string | null;
  entityId?: string | null;
  wikiLang: string;
  onEntity: (entityId: string) => void;
}): Promise<IngestJobResponse | null> {
  const lockKey = `explorer:${input.subject}:${input.qid ?? ""}`;
  if (ingestLock === lockKey) return null;
  ingestLock = lockKey;
  try {
    const job = await startExplorerIngest({
      subject: input.subject,
      qid: input.qid,
      live: true,
      maxDocuments: EXPLORER_INGEST_MAX_DOCUMENTS,
      wikiLang: input.wikiLang,
    });
    let bound = input.entityId;
    const bind = (id?: string | null) => {
      if (!id || bound === id) return;
      bound = id;
      input.onEntity(id);
    };
    bind(job.entity_id);
    const result = await pollIngestJob(job.job_id, (tick) => bind(tick.entity_id));
    bind(result.entity_id);
    return result;
  } finally {
    if (ingestLock === lockKey) ingestLock = null;
  }
}

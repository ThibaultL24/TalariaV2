// web/src/hooks/use-progressive-ingest.ts
import { useCallback, useEffect, useRef, useState } from "react";
import {
  fetchExplorerStatus,
  startExplorerIngest,
  EXPLORER_INGEST_MAX_DOCUMENTS,
  type ExplorerIngestStatus,
} from "@/lib/api";

const STATUS_POLL_MS = 1500;
const INGEST_TIMEOUT_MS = 45 * 60 * 1000;

export interface ProgressiveIngestState {
  jobId: string | null;
  status: ExplorerIngestStatus | null;
  isRunning: boolean;
  error: string | null;
}

interface UseProgressiveIngestOptions {
  onEntityResolved?: (entityId: string) => void;
  onCountsChanged?: (timeline: number, mapPins: number) => void;
}

export function useProgressiveIngest(opts: UseProgressiveIngestOptions = {}) {
  const [state, setState] = useState<ProgressiveIngestState>({
    jobId: null,
    status: null,
    isRunning: false,
    error: null,
  });
  
  const lastCountsRef = useRef({ timeline: 0, mapPins: 0 });
  const pollingRef = useRef<number | null>(null);
  const deadlineRef = useRef<number>(0);

  const stopPolling = useCallback(() => {
    if (pollingRef.current !== null) {
      window.clearInterval(pollingRef.current);
      pollingRef.current = null;
    }
  }, []);

  const pollStatus = useCallback(async (jobId: string) => {
    if (Date.now() > deadlineRef.current) {
      stopPolling();
      setState((prev) => ({
        ...prev,
        isRunning: false,
        error: "timeout",
      }));
      return;
    }

    try {
      const status = await fetchExplorerStatus(jobId);
      
      setState((prev) => ({
        ...prev,
        status,
        isRunning: !status.is_done,
        error: status.error ?? null,
      }));

      if (status.entity_id && opts.onEntityResolved) {
        opts.onEntityResolved(status.entity_id);
      }

      const newCounts = {
        timeline: status.timeline_events,
        mapPins: status.map_pins,
      };
      if (
        newCounts.timeline !== lastCountsRef.current.timeline ||
        newCounts.mapPins !== lastCountsRef.current.mapPins
      ) {
        lastCountsRef.current = newCounts;
        opts.onCountsChanged?.(newCounts.timeline, newCounts.mapPins);
      }

      if (status.is_done) {
        stopPolling();
      }
    } catch (err) {
      setState((prev) => ({
        ...prev,
        isRunning: false,
        error: err instanceof Error ? err.message : "polling failed",
      }));
      stopPolling();
    }
  }, [opts, stopPolling]);

  const startIngest = useCallback(
    async (input: {
      subject: string;
      qid?: string | null;
      wikiLang: string;
    }) => {
      stopPolling();
      setState({
        jobId: null,
        status: null,
        isRunning: true,
        error: null,
      });
      lastCountsRef.current = { timeline: 0, mapPins: 0 };

      try {
        const job = await startExplorerIngest({
          subject: input.subject,
          qid: input.qid,
          live: true,
          maxDocuments: EXPLORER_INGEST_MAX_DOCUMENTS,
          wikiLang: input.wikiLang,
        });

        setState((prev) => ({
          ...prev,
          jobId: job.job_id,
          isRunning: true,
        }));

        if (job.entity_id) {
          opts.onEntityResolved?.(job.entity_id);
        }

        deadlineRef.current = Date.now() + INGEST_TIMEOUT_MS;
        pollingRef.current = window.setInterval(() => {
          pollStatus(job.job_id);
        }, STATUS_POLL_MS);
        
        // Immediate first poll
        pollStatus(job.job_id);
        
        return job;
      } catch (err) {
        setState((prev) => ({
          ...prev,
          isRunning: false,
          error: err instanceof Error ? err.message : "start failed",
        }));
        return null;
      }
    },
    [opts, pollStatus, stopPolling],
  );

  const cancel = useCallback(() => {
    stopPolling();
    setState((prev) => ({
      ...prev,
      isRunning: false,
    }));
  }, [stopPolling]);

  useEffect(() => {
    return () => stopPolling();
  }, [stopPolling]);

  return {
    ...state,
    startIngest,
    cancel,
    phase: state.status?.phase ?? null,
    timelineEvents: state.status?.timeline_events ?? 0,
    mapPins: state.status?.map_pins ?? 0,
    preciseDates: state.status?.precise_dates ?? 0,
    preciseCoords: state.status?.precise_coords ?? 0,
    evidenceCount: state.status?.evidence_count ?? 0,
    wikiPages: state.status?.wiki_pages ?? 0,
    elapsedMs: state.status?.elapsed_ms ?? null,
  };
}

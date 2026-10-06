// web/src/hooks/use-interaction-summary.ts
import { useCallback, useEffect, useState } from "react";
import {
  fetchInteractionSummary,
  type InteractionSummary,
  type InteractionTargetType,
} from "@/lib/api";

export function useInteractionSummary(
  targetType: InteractionTargetType | null,
  targetIds: string[],
) {
  const [summaries, setSummaries] = useState<InteractionSummary[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    if (!targetType || targetIds.length === 0) {
      setSummaries([]);
      return;
    }
    setIsLoading(true);
    setError(null);
    try {
      const data = await fetchInteractionSummary(targetType, targetIds);
      setSummaries(data.summaries);
    } catch (err) {
      setError(err instanceof Error ? err.message : "request_failed");
    } finally {
      setIsLoading(false);
    }
  }, [targetType, targetIds.join(",")]);

  useEffect(() => {
    void reload();
  }, [reload]);

  return { summaries, isLoading, error, reload };
}

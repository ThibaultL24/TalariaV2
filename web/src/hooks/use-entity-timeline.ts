import { useEffect, useMemo } from "react";
import { useInfiniteQuery, useQueryClient } from "@tanstack/react-query";
import { getEntityView, type Page } from "@/lib/entity-views";
import type { TimelineEvent } from "@/lib/api";

const PAGE_SIZE = 200;
const CACHE_FRESHNESS_MS = 30_000;

/** The person, filters and locale identify pages; new ingestion invalidates in place. */
export function useEntityTimeline(
  entityId: string,
  filters: string,
  locale: string,
  enabled: boolean,
  revision: number,
) {
  const queryClient = useQueryClient();
  const query = useInfiniteQuery({
    queryKey: ["istoria", "timeline", entityId, filters, locale],
    enabled: Boolean(entityId) && enabled,
    initialPageParam: null as string | null,
    queryFn: ({ pageParam, signal }) => {
      const params = new URLSearchParams(filters);
      params.set("limit", String(PAGE_SIZE));
      params.set("resolution", "detail");
      params.set("lang", locale);
      if (pageParam) params.set("cursor", pageParam);
      return getEntityView<Page>(entityId, "timeline", params, signal);
    },
    getNextPageParam: (lastPage) => lastPage.pagination?.next_cursor ?? undefined,
    staleTime: CACHE_FRESHNESS_MS,
    refetchOnWindowFocus: false,
    retry: false,
  });

  // Refresh all already-loaded pages without clearing them as person ingestion progresses.
  // Debounce frequent ingestion ticks to avoid an HTTP request for every updated count.
  useEffect(() => {
    if (!entityId || !enabled || revision === 0) return;
    const timer = setTimeout(() => {
      void queryClient.invalidateQueries({
        queryKey: ["istoria", "timeline", entityId],
        refetchType: "active",
      });
    }, 600);
    return () => clearTimeout(timer);
  }, [enabled, entityId, queryClient, revision]);

  const events: TimelineEvent[] = useMemo(
    () => query.data?.pages.flatMap((page) => page.events) ?? [],
    [query.data],
  );

  return {
    events,
    hasMore: Boolean(query.hasNextPage),
    loadMore: query.fetchNextPage,
    loading: query.isPending || query.isFetchingNextPage,
    refreshing: query.isFetching && !query.isPending && !query.isFetchingNextPage,
    isFetchingNextPage: query.isFetchingNextPage,
    error: query.error,
  };
}

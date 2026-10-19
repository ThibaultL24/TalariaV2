import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import { useEntityTimeline } from "./use-entity-timeline";

const response = (items: string[], cursor: string | null) => ({
  events: items.map((id) => ({ id, entity_id: "person-1", title: id, event_type: "travel", map_eligible: false })),
  count: items.length,
  pagination: { next_cursor: cursor },
});

function setup() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return { client, wrapper };
}

afterEach(() => vi.unstubAllGlobals());

describe("useEntityTimeline", () => {
  it("starts with one page and loads the next page only on demand", async () => {
    const fetchMock = vi.fn(async (url: string) => ({
      ok: true,
      json: async () => String(url).includes("cursor=next")
        ? response(["third"], null)
        : response(["first", "second"], "next"),
    }));
    vi.stubGlobal("fetch", fetchMock);
    const { wrapper } = setup();
    const { result } = renderHook(() => useEntityTimeline("person-1", "", "fr", true, 0), { wrapper });
    await waitFor(() => expect(result.current.events.map((event) => event.id)).toEqual(["first", "second"]));
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(result.current.hasMore).toBe(true);
    await act(async () => { await result.current.loadMore(); });
    expect(result.current.events.map((event) => event.id)).toEqual(["first", "second", "third"]);
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("keeps events rendered while ingestion refreshes the same entity", async () => {
    let resolveRefresh: ((value: unknown) => void) | undefined;
    let calls = 0;
    vi.stubGlobal("fetch", vi.fn((_url: string) => {
      calls++;
      if (calls === 1) {
        return Promise.resolve({ ok: true, json: async () => response(["existing"], null) });
      }
      return new Promise((resolve) => { resolveRefresh = resolve; });
    }));
    const { wrapper, client } = setup();
    const { result } = renderHook(() => useEntityTimeline("person-1", "", "fr", true, 0), { wrapper });
    await waitFor(() => expect(result.current.events[0]?.id).toBe("existing"));
    await act(async () => {
      void client.invalidateQueries({ queryKey: ["istoria", "timeline", "person-1"] });
    });
    await waitFor(() => expect(calls).toBe(2));
    expect(result.current.events[0]?.id).toBe("existing");
    await act(async () => {
      resolveRefresh?.({ ok: true, json: async () => response(["existing", "new"], null) });
    });
    await waitFor(() => expect(result.current.events.map((event) => event.id)).toEqual(["existing", "new"]));
  });

  it("never shows another person's cached events", async () => {
    vi.stubGlobal("fetch", vi.fn(async (url: string) => ({
      ok: true,
      json: async () => response([String(url).includes("person-2") ? "second-person" : "first-person"], null),
    })));
    const { wrapper } = setup();
    const { result, rerender } = renderHook(
      ({ id }) => useEntityTimeline(id, "", "fr", true, 0),
      { initialProps: { id: "person-1" }, wrapper },
    );
    await waitFor(() => expect(result.current.events[0]?.id).toBe("first-person"));
    rerender({ id: "person-2" });
    expect(result.current.events.some((event) => event.id === "first-person")).toBe(false);
    await waitFor(() => expect(result.current.events[0]?.id).toBe("second-person"));
  });
});

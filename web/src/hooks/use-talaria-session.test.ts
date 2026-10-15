// web/src/hooks/use-talaria-session.test.ts
import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useTalariaSession } from "./use-talaria-session";

const fetchMock = vi.fn();

describe("useTalariaSession", () => {
  beforeEach(() => {
    fetchMock.mockReset();
    vi.stubGlobal("fetch", fetchMock);
  });

  it("starts unauthenticated without a session cookie", async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: async () => ({ authenticated: false }),
    });
    const { result } = renderHook(() => useTalariaSession());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.authenticated).toBe(false);
    expect(result.current.user).toBeNull();
  });

  it("signs in via challenge + signature then reads the user", async () => {
    fetchMock.mockImplementation(async (url: string, init?: RequestInit) => {
      const path = String(url);
      if (path.includes("/auth/me")) {
        return { ok: true, json: async () => ({ authenticated: false }) };
      }
      if (path.includes("/challenge")) {
        return {
          ok: true,
          json: async () => ({ challenge_id: "cid", message: "SIWE message" }),
        };
      }
      if (path.includes("/verify")) {
        expect(init?.credentials).toBe("include");
        return {
          ok: true,
          json: async () => ({
            user: { id: "user-1", wallet_address: "0xabc" },
          }),
        };
      }
      throw new Error(path);
    });
    const { result } = renderHook(() => useTalariaSession());
    await act(async () => {
      await result.current.signIn({
        address: "0xabc",
        chainId: 13579,
        signMessage: async (message) => {
          expect(message).toBe("SIWE message");
          return "0xsig";
        },
      });
    });
    expect(result.current.authenticated).toBe(true);
    expect(result.current.user?.id).toBe("user-1");
  });
});

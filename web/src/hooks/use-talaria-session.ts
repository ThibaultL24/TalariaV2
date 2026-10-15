// web/src/hooks/use-talaria-session.ts
import { useCallback, useEffect, useState } from "react";
import {
  fetchTalariaMe,
  logoutTalariaSession,
  requestWalletChallenge,
  verifyWalletChallenge,
  type TalariaAuthUser,
} from "@/lib/api";

interface SignInArgs {
  address: string;
  chainId: number;
  signMessage: (message: string) => Promise<string>;
}

export function useTalariaSession() {
  const [user, setUser] = useState<TalariaAuthUser | null>(null);
  const [loading, setLoading] = useState(true);
  const [signingIn, setSigningIn] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const me = await fetchTalariaMe();
    setUser(me.authenticated && me.user ? me.user : null);
  }, []);

  useEffect(() => {
    let cancelled = false;
    fetchTalariaMe()
      .then((me) => {
        if (!cancelled) setUser(me.authenticated && me.user ? me.user : null);
      })
      .catch(() => {
        if (!cancelled) setUser(null);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const signIn = useCallback(async ({ address, chainId, signMessage }: SignInArgs) => {
    setSigningIn(true);
    setError(null);
    try {
      const challenge = await requestWalletChallenge(address, chainId);
      const signature = await signMessage(challenge.message);
      const result = await verifyWalletChallenge(
        challenge.challenge_id,
        challenge.message,
        signature,
      );
      setUser(result.user);
    } catch (err) {
      setError(err instanceof Error ? err.message : "sign_in_failed");
      throw err;
    } finally {
      setSigningIn(false);
    }
  }, []);

  const logout = useCallback(async () => {
    await logoutTalariaSession();
    setUser(null);
  }, []);

  return {
    authenticated: Boolean(user),
    user,
    loading,
    signingIn,
    error,
    signIn,
    logout,
    refresh,
  };
}

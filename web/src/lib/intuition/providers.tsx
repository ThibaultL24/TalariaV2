// web/src/lib/intuition/providers.tsx
import { type ReactNode, useEffect } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { WagmiProvider, useChainId } from "wagmi";
import { applyIntuitionSdkConfig, intuitionQueryClient, wagmiConfig } from "./clients";
import { isSupportedIntuitionChain } from "./config";
import { appIntuitionChain } from "./network";

function IntuitionSdkSync({ children }: { children: ReactNode }) {
  const chainId = useChainId();
  useEffect(() => {
    const id = isSupportedIntuitionChain(chainId) ? chainId : appIntuitionChain().id;
    applyIntuitionSdkConfig(id);
  }, [chainId]);
  return children;
}

export function IntuitionProviders({ children }: { children: ReactNode }) {
  return (
    <WagmiProvider config={wagmiConfig}>
      <QueryClientProvider client={intuitionQueryClient}>
        <IntuitionSdkSync>{children}</IntuitionSdkSync>
      </QueryClientProvider>
    </WagmiProvider>
  );
}

// web/src/lib/intuition/clients.ts
import { QueryClient } from "@tanstack/react-query";
import { configureClient } from "@0xintuition/graphql";
import { configureSdk, intuitionMainnet, intuitionTestnet } from "@0xintuition/sdk";
import { http, createConfig } from "wagmi";
import { injected } from "wagmi/connectors";
import { getIntuitionApiUrlForChain } from "./config";

export function applyIntuitionSdkConfig(chainId: number): void {
  const apiUrl = getIntuitionApiUrlForChain(chainId);
  configureSdk({ apiUrl });
  configureClient({ apiUrl });
}

export const wagmiConfig = createConfig({
  chains: [intuitionTestnet, intuitionMainnet],
  connectors: [injected()],
  transports: {
    [intuitionTestnet.id]: http(intuitionTestnet.rpcUrls.default.http[0]),
    [intuitionMainnet.id]: http(intuitionMainnet.rpcUrls.default.http[0]),
  },
});

export const intuitionQueryClient = new QueryClient();

// web/src/hooks/use-intuition-wallet.ts
import { useAccount, useChainId, usePublicClient, useWalletClient } from "wagmi";
import { isSupportedIntuitionChain } from "@/lib/intuition/config";

export function useIntuitionWallet() {
  const { address, isConnected } = useAccount();
  const chainId = useChainId();
  const publicClient = usePublicClient();
  const { data: walletClient } = useWalletClient();
  return {
    address,
    chainId,
    walletClient,
    publicClient,
    isConnected,
    isSupportedChain: isSupportedIntuitionChain(chainId),
  };
}

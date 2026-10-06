// web/src/components/wallet/wallet-connect-button.tsx
import { useConnect, useDisconnect, useSwitchChain } from "wagmi";
import { useIntuitionWallet } from "@/hooks/use-intuition-wallet";
import { useTalariaSession } from "@/hooks/use-talaria-session";
import { useI18n } from "@/lib/i18n";
import { appIntuitionChain } from "@/lib/intuition/network";
import { shortenAddress } from "@/lib/shorten-address";

interface WalletConnectButtonProps {
  compact?: boolean;
}

export function WalletConnectButton({ compact = false }: WalletConnectButtonProps) {
  const { t } = useI18n();
  const { address, isConnected, isSupportedChain, chainId, walletClient } = useIntuitionWallet();
  const { authenticated, signingIn, signIn } = useTalariaSession();
  const { connect, connectors, isPending, error } = useConnect();
  const { disconnect } = useDisconnect();
  const { switchChain } = useSwitchChain();
  const targetChain = appIntuitionChain();

  if (isConnected && address && !isSupportedChain) {
    return (
      <div className="wallet-connect wallet-connect--linked">
        <span className="wallet-connect__error" role="alert">
          {t.walletWrongNetwork}
        </span>
        <button
          type="button"
          className="wallet-connect__btn"
          onClick={() => switchChain({ chainId: targetChain.id })}
        >
          {t.walletSwitchNetwork}
        </button>
      </div>
    );
  }

  if (isConnected && address) {
    return (
      <div className="wallet-connect wallet-connect--linked">
        <span className="wallet-connect__addr" title={address}>
          {shortenAddress(address)}
        </span>
        {!authenticated ? (
          <button
            type="button"
            className="wallet-connect__btn"
            disabled={signingIn || !walletClient}
            title={t.talariaSignInHint}
            onClick={() => {
              if (!walletClient) return;
              void signIn({
                address,
                chainId,
                signMessage: async (message) => walletClient.signMessage({ message }),
              });
            }}
          >
            {signingIn ? t.walletConnecting : t.talariaSignIn}
          </button>
        ) : null}
        <button
          type="button"
          className="wallet-connect__btn wallet-connect__btn--ghost"
          onClick={() => disconnect()}
        >
          {compact ? "×" : t.walletDisconnect}
        </button>
      </div>
    );
  }

  const connector = connectors[0];

  return (
    <div className="wallet-connect">
      <button
        type="button"
        className="wallet-connect__btn"
        disabled={isPending || !connector}
        title={t.walletConnectHint}
        onClick={() => {
          if (!connector) return;
          connect({ connector, chainId: targetChain.id });
        }}
      >
        {isPending ? t.walletConnecting : t.walletConnect}
      </button>
      {error || !connector ? (
        <span className="wallet-connect__error" role="alert">
          {connector ? t.walletConnectFailed : t.walletNoProvider}
        </span>
      ) : null}
    </div>
  );
}

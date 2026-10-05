// web/src/components/wallet/wallet-connect-button.tsx
import { useI18n } from "@/lib/i18n";
import { shortenAddress } from "@/lib/wallet";
import { useWalletStore } from "@/stores/wallet-store";

interface WalletConnectButtonProps {
  compact?: boolean;
}

export function WalletConnectButton({ compact = false }: WalletConnectButtonProps) {
  const { t } = useI18n();
  const address = useWalletStore((s) => s.address);
  const connecting = useWalletStore((s) => s.connecting);
  const error = useWalletStore((s) => s.error);
  const connect = useWalletStore((s) => s.connect);
  const disconnect = useWalletStore((s) => s.disconnect);

  const errorLabel =
    error === "no_wallet_provider" ? t.walletNoProvider : t.walletConnectFailed;

  if (address) {
    return (
      <div className="wallet-connect wallet-connect--linked">
        <span className="wallet-connect__addr" title={address}>
          {shortenAddress(address)}
        </span>
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

  return (
    <div className="wallet-connect">
      <button
        type="button"
        className="wallet-connect__btn"
        disabled={connecting}
        onClick={() => void connect()}
        title={t.walletConnectHint}
      >
        {connecting ? t.walletConnecting : t.walletConnect}
      </button>
      {error ? (
        <span className="wallet-connect__error" role="alert">
          {errorLabel}
        </span>
      ) : null}
    </div>
  );
}

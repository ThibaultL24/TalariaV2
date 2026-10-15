// web/src/components/wallet/intuition-wallet-panel.tsx
import { useBalance, useSwitchChain } from "wagmi";
import { IntuitionConnectButton } from "@/components/wallet/intuition-connect-button";
import { IntuitionWalletAvatar } from "@/components/wallet/intuition-wallet-avatar";
import { useIntuitionWallet } from "@/hooks/use-intuition-wallet";
import { formatTokenBalance } from "@/lib/intuition/format-balance";
import { appIntuitionChain } from "@/lib/intuition/network";
import { useI18n } from "@/lib/i18n";
import { shortenAddress } from "@/lib/shorten-address";

export function IntuitionWalletPanel() {
  const { t } = useI18n();
  const { address, isConnected, isSupportedChain } = useIntuitionWallet();
  const targetChain = appIntuitionChain();
  const { switchChain, isPending: isSwitching } = useSwitchChain();
  const { data: balance, isLoading: isBalanceLoading } = useBalance({
    address,
    chainId: targetChain.id,
    query: { enabled: Boolean(address && isConnected) },
  });

  return (
    <div className="agora-intuition-wallet rounded-xl border border-(--agora-intuition-border) bg-(--agora-intuition-surface) p-5 shadow-(--agora-intuition-shadow)">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="text-[10px] font-semibold uppercase tracking-[0.14em] text-(--agora-intuition-gold)">
            {t.walletPanelKicker}
          </p>
          <h3
            className="mt-1 text-base font-semibold tracking-tight text-(--agora-intuition-heading)"
            style={{ fontFamily: "var(--font-display)" }}
          >
            {t.walletPanelTitle}
          </h3>
          <p className="mt-2 max-w-prose text-sm leading-relaxed text-(--agora-intuition-muted)">
            {t.walletPanelLead}
          </p>
        </div>
        <IntuitionConnectButton variant="panel" />
      </div>

      {isConnected && address ? (
        <div className="mt-5 flex items-center gap-3 rounded-lg border border-(--agora-intuition-border) bg-(--color-bg-surface) px-3.5 py-3">
          <IntuitionWalletAvatar address={address} size={40} />
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-semibold text-(--agora-intuition-heading)">
              {shortenAddress(address)}
            </p>
            {!isBalanceLoading && balance != null ? (
              <p className="mt-0.5 text-xs tabular-nums text-(--agora-intuition-muted)">
                {formatTokenBalance(balance.value, balance.decimals)} {balance.symbol}
              </p>
            ) : (
              <p className="mt-0.5 text-xs text-(--agora-intuition-dim)">{t.walletConnected}</p>
            )}
          </div>
        </div>
      ) : null}

      {isConnected && !isSupportedChain ? (
        <div
          className="mt-4 flex flex-col gap-2 rounded-lg border border-amber-500/35 bg-amber-500/10 px-3 py-2.5 sm:flex-row sm:items-center sm:justify-between"
          role="status"
        >
          <p className="text-xs leading-snug text-(--color-text-primary)">{t.walletWrongNetworkDetail}</p>
          <button
            type="button"
            disabled={isSwitching}
            className="shrink-0 rounded-md border border-amber-400/40 bg-amber-500/15 px-2.5 py-1 text-xs font-semibold disabled:opacity-50"
            onClick={() => switchChain({ chainId: targetChain.id })}
          >
            {t.walletSwitchNetwork}
          </button>
        </div>
      ) : null}
    </div>
  );
}

// web/src/components/wallet/intuition-connect-button.tsx
import { useEffect, useRef, useState } from "react";
import { useBalance, useDisconnect, useSwitchChain } from "wagmi";
import { IntuitionConnectModal } from "@/components/wallet/intuition-connect-modal";
import { IntuitionWalletAvatar } from "@/components/wallet/intuition-wallet-avatar";
import { useIntuitionWallet } from "@/hooks/use-intuition-wallet";
import { useTalariaSession } from "@/hooks/use-talaria-session";
import { formatTokenBalance } from "@/lib/intuition/format-balance";
import { appIntuitionChain } from "@/lib/intuition/network";
import { useI18n } from "@/lib/i18n";
import { shortenAddress } from "@/lib/shorten-address";

const INTUITION_NETWORK_HUB_URL = "https://portal.intuition.systems/";

interface IntuitionConnectButtonProps {
  variant?: "navbar" | "panel";
  className?: string;
}

export function IntuitionConnectButton({
  variant = "navbar",
  className = "",
}: IntuitionConnectButtonProps) {
  const { t } = useI18n();
  const [connectOpen, setConnectOpen] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const shellRef = useRef<HTMLDivElement>(null);
  const { address, isConnected, isSupportedChain, chainId, walletClient } = useIntuitionWallet();
  const session = useTalariaSession();
  const targetChain = appIntuitionChain();
  const { disconnect, isPending: isDisconnecting } = useDisconnect();
  const { switchChain, isPending: isSwitching } = useSwitchChain();
  const { data: balance, isLoading: isBalanceLoading } = useBalance({
    address,
    chainId: targetChain.id,
    query: { enabled: Boolean(address && isConnected) },
  });

  useEffect(() => {
    if (!menuOpen) return;
    function onPointerDown(event: MouseEvent) {
      if (shellRef.current && !shellRef.current.contains(event.target as Node)) {
        setMenuOpen(false);
      }
    }
    window.addEventListener("pointerdown", onPointerDown);
    return () => window.removeEventListener("pointerdown", onPointerDown);
  }, [menuOpen]);

  const shellClass = variant === "navbar" ? "relative shrink-0" : `relative ${className}`;

  if (isConnected && address) {
    const short = shortenAddress(address);
    const triggerClass =
      variant === "navbar"
        ? "intuition-wallet-trigger intuition-wallet-trigger--navbar"
        : "intuition-wallet-trigger intuition-wallet-trigger--panel";

    return (
      <div className={shellClass} ref={shellRef}>
        <button
          type="button"
          className={triggerClass}
          onClick={() => setMenuOpen((open) => !open)}
          aria-expanded={menuOpen}
          aria-haspopup="true"
        >
          <IntuitionWalletAvatar address={address} size={variant === "navbar" ? 24 : 28} />
          <span>{!isSupportedChain ? t.walletSwitchNetwork : short}</span>
          <span className="text-[10px] opacity-50" aria-hidden>
            ▾
          </span>
        </button>
        {menuOpen ? (
          <div className="intuition-wallet-menu" role="menu">
            <div className="intuition-wallet-menu__header">
              <IntuitionWalletAvatar address={address} size={36} />
              <div className="min-w-0">
                <p className="truncate text-sm font-semibold">{short}</p>
                <p className="intuition-wallet-menu__address" title={address}>
                  {address}
                </p>
              </div>
            </div>
            <div className="intuition-wallet-menu__meta">
              <p>{targetChain.name}</p>
              {!isBalanceLoading && balance != null ? (
                <p className="intuition-wallet-menu__balance mt-1">
                  {formatTokenBalance(balance.value, balance.decimals)} {balance.symbol}
                </p>
              ) : null}
            </div>
            <div className="intuition-wallet-menu__actions">
              {!isSupportedChain ? (
                <button
                  type="button"
                  role="menuitem"
                  disabled={isSwitching}
                  className="intuition-wallet-menu__action intuition-wallet-menu__action--warn"
                  onClick={() => {
                    switchChain({ chainId: targetChain.id });
                    setMenuOpen(false);
                  }}
                >
                  {t.walletSwitchNetwork}
                </button>
              ) : null}
              {!session.authenticated ? (
                <button
                  type="button"
                  role="menuitem"
                  disabled={session.signingIn || !walletClient}
                  className="intuition-wallet-menu__action"
                  title={t.talariaSignInHint}
                  onClick={() => {
                    if (!walletClient) return;
                    void session.signIn({
                      address,
                      chainId,
                      signMessage: async (message) => walletClient.signMessage({ message }),
                    });
                    setMenuOpen(false);
                  }}
                >
                  {session.signingIn ? t.walletConnecting : t.talariaSignIn}
                </button>
              ) : null}
              <a
                href={INTUITION_NETWORK_HUB_URL}
                target="_blank"
                rel="noreferrer"
                role="menuitem"
                className="intuition-wallet-menu__action"
              >
                {t.walletNetworkHub}
              </a>
              <button
                type="button"
                role="menuitem"
                disabled={isDisconnecting}
                className="intuition-wallet-menu__action intuition-wallet-menu__action--danger"
                onClick={() => {
                  disconnect();
                  setMenuOpen(false);
                }}
              >
                {t.walletDisconnect}
              </button>
            </div>
          </div>
        ) : null}
      </div>
    );
  }

  const connectTriggerClass =
    variant === "navbar"
      ? "intuition-wallet-trigger intuition-wallet-trigger--navbar intuition-wallet-trigger--connect"
      : "intuition-wallet-trigger intuition-wallet-trigger--panel intuition-wallet-trigger--connect";

  return (
    <div className={shellClass}>
      <button type="button" className={connectTriggerClass} onClick={() => setConnectOpen(true)}>
        {t.walletConnect}
      </button>
      <IntuitionConnectModal
        open={connectOpen}
        onClose={() => setConnectOpen(false)}
        chainId={targetChain.id}
        networkLabel={targetChain.name}
      />
    </div>
  );
}

// web/src/components/wallet/intuition-connect-modal.tsx
import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { useConnect, useConnectors } from "wagmi";
import {
  INTUITION_WALLET_OPTIONS,
  resolveConnectorForOption,
} from "@/lib/intuition/wallet-options";
import { useI18n } from "@/lib/i18n";

const WALLET_LEARN_URL = "https://ethereum.org/en/wallets/";
const WALLET_GET_URL = "https://ethereum.org/en/wallets/find-wallet/";

interface IntuitionConnectModalProps {
  open: boolean;
  onClose: () => void;
  chainId: number;
  networkLabel: string;
}

function walletIconLabel(name: string): string {
  const parts = name.split(/\s+/).filter(Boolean);
  if (parts.length >= 2) return `${parts[0][0]}${parts[1][0]}`.slice(0, 2);
  return name.slice(0, 2);
}

export function IntuitionConnectModal({
  open,
  onClose,
  chainId,
  networkLabel,
}: IntuitionConnectModalProps) {
  const { t } = useI18n();
  const dialogRef = useRef<HTMLDivElement>(null);
  const connectors = useConnectors();
  const { connect, isPending, error } = useConnect();

  useEffect(() => {
    if (!open) return;
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    document.addEventListener("keydown", onKey);
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = previous;
    };
  }, [open, onClose]);

  if (!open) return null;

  return createPortal(
    <div
      className="intuition-wallet-modal__backdrop"
      role="presentation"
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        ref={dialogRef}
        className="intuition-wallet-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="intuition-connect-title"
        onClick={(event) => event.stopPropagation()}
      >
        <section className="intuition-wallet-modal__wallets">
          <div className="intuition-wallet-modal__head">
            <div>
              <h2 id="intuition-connect-title" className="intuition-wallet-modal__title">
                {t.walletConnectModalTitle}
              </h2>
              <p className="mt-0.5 text-xs text-(--color-text-muted)">{t.walletConnect}</p>
            </div>
            <button
              type="button"
              className="intuition-wallet-modal__close"
              aria-label={t.close}
              onClick={onClose}
            >
              ×
            </button>
          </div>
          <ul className="intuition-wallet-modal__list">
            {INTUITION_WALLET_OPTIONS.map((option) => {
              const connector = resolveConnectorForOption(option, connectors);
              const unavailable = connector == null;
              return (
                <li key={option.id}>
                  <button
                    type="button"
                    className="intuition-wallet-modal__option"
                    disabled={isPending || unavailable}
                    onClick={() => {
                      if (!connector) return;
                      connect({ connector, chainId });
                      onClose();
                    }}
                  >
                    <span className="intuition-wallet-modal__icon" aria-hidden>
                      {walletIconLabel(option.name)}
                    </span>
                    {option.name}
                  </button>
                </li>
              );
            })}
          </ul>
          {error ? (
            <p className="px-4 pb-3 text-xs text-(--color-trust-low)" role="alert">
              {error.message}
            </p>
          ) : null}
          <p className="px-4 pb-3 text-[11px] leading-snug text-(--color-text-muted)">
            {t.walletConnectHintNetwork(networkLabel)}
          </p>
        </section>
        <aside className="intuition-wallet-modal__edu">
          <h3 className="intuition-wallet-modal__edu-title">{t.walletEduTitle}</h3>
          <div className="intuition-wallet-modal__edu-block">
            <div className="intuition-wallet-modal__edu-art">
              <span aria-hidden>◆</span>
            </div>
            <h3>{t.walletEduAssetsTitle}</h3>
            <p>{t.walletEduAssetsBody}</p>
          </div>
          <div className="intuition-wallet-modal__edu-block">
            <div className="intuition-wallet-modal__edu-art">
              <span aria-hidden>◎</span>
            </div>
            <h3>{t.walletEduLoginTitle}</h3>
            <p>{t.walletEduLoginBody}</p>
          </div>
          <div className="intuition-wallet-modal__cta">
            <a
              href={WALLET_GET_URL}
              target="_blank"
              rel="noreferrer"
              className="intuition-wallet-modal__cta-primary"
            >
              {t.walletGetWallet}
            </a>
            <a
              href={WALLET_LEARN_URL}
              target="_blank"
              rel="noreferrer"
              className="intuition-wallet-modal__cta-link"
            >
              {t.walletLearnMore}
            </a>
          </div>
        </aside>
      </div>
    </div>,
    document.body,
  );
}

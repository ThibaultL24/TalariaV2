// web/src/components/agora/claim-action-bar.tsx
import { useState } from "react";
import { useSwitchChain } from "wagmi";
import {
  postInteraction,
  syncClaimSignal,
  fetchClaimIntuition,
  type InteractionAction,
  type InteractionSummary,
} from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import { useIntuitionWallet } from "@/hooks/use-intuition-wallet";
import { useTalariaSession } from "@/hooks/use-talaria-session";
import { appIntuitionChain } from "@/lib/intuition/network";
import {
  TRUST_PRESETS,
  depositClaimSignal,
  formatTrustAmount,
  parseTrustAmount,
  previewClaimSignal,
  type ClaimSignalAction,
  type ClaimSignalPreview,
} from "@/lib/intuition/signals";

export type TxPhase =
  | "idle"
  | "preparing"
  | "awaiting_signature"
  | "submitted"
  | "confirming"
  | "confirmed"
  | "failed";

interface ClaimActionBarProps {
  claimId: string;
  claimText: string;
  summary?: InteractionSummary;
  intuitionStatusLabel?: string | null;
  onChanged?: () => void;
}

function countOf(summary: InteractionSummary | undefined, action: string): number {
  return summary?.counts?.[action] ?? 0;
}

function walletsMatch(session: string | null | undefined, wagmi: string | undefined): boolean {
  if (!session || !wagmi) return false;
  return session.toLowerCase() === wagmi.toLowerCase();
}

export function ClaimActionBar({
  claimId,
  claimText,
  summary,
  intuitionStatusLabel,
  onChanged,
}: ClaimActionBarProps) {
  const { t } = useI18n();
  const session = useTalariaSession();
  const wallet = useIntuitionWallet();
  const { switchChain } = useSwitchChain();
  const requiredChain = appIntuitionChain();

  const [phase, setPhase] = useState<TxPhase>("idle");
  const [pendingAction, setPendingAction] = useState<ClaimSignalAction | null>(null);
  const [amount, setAmount] = useState("1");
  const [custom, setCustom] = useState("");
  const [preview, setPreview] = useState<ClaimSignalPreview | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [syncRetryHash, setSyncRetryHash] = useState<string | null>(null);
  const mine = summary?.mine ?? [];

  function phaseLabel(value: TxPhase): string {
    switch (value) {
      case "preparing":
        return t.signalPreparing;
      case "awaiting_signature":
        return t.signalAwaitingSignature;
      case "submitted":
        return t.signalSubmitted;
      case "confirming":
        return t.signalConfirming;
      case "confirmed":
        return t.signalConfirmed;
      case "failed":
        return t.signalFailed;
      default:
        return "";
    }
  }

  async function onUncertain() {
    setMessage(null);
    if (!session.authenticated) {
      setMessage(t.signalSignInRequired);
      return;
    }
    try {
      await postInteraction({
        action: "uncertain",
        target_type: "claim",
        target_id: claimId,
        visibility: "public",
      });
      onChanged?.();
    } catch (err) {
      setMessage(err instanceof Error ? err.message : t.signalFailed);
    }
  }

  async function startEconomic(action: ClaimSignalAction) {
    setMessage(null);
    setSyncRetryHash(null);
    setPreview(null);
    if (!session.authenticated) {
      setMessage(t.signalSignInRequired);
      return;
    }
    if (!wallet.isConnected || !wallet.address) {
      setMessage(t.walletConnectForStance);
      return;
    }
    if (!walletsMatch(session.user?.wallet_address, wallet.address)) {
      setMessage(t.signalWalletMismatch);
      return;
    }
    if (!wallet.isSupportedChain || wallet.chainId !== requiredChain.id) {
      setMessage(t.walletWrongNetwork);
      return;
    }
    setPhase("preparing");
    try {
      const binding = await fetchClaimIntuition(claimId);
      if (binding.status !== "ready") {
        setPhase("idle");
        setMessage(t.signalNotPublished);
        return;
      }
      setPendingAction(action);
      setPhase("idle");
    } catch (err) {
      setPhase("failed");
      setMessage(err instanceof Error ? err.message : t.signalFailed);
    }
  }

  async function runPreview() {
    if (!pendingAction) return;
    setPhase("preparing");
    setMessage(null);
    try {
      const binding = await fetchClaimIntuition(claimId);
      if (binding.status !== "ready") {
        setPhase("idle");
        setPendingAction(null);
        setMessage(t.signalNotPublished);
        return;
      }
      const assets = parseTrustAmount(amount === "custom" ? custom : amount);
      if (!wallet.publicClient || !wallet.walletClient || !wallet.address) {
        throw new Error(t.signalFailed);
      }
      const next = await previewClaimSignal({
        clients: {
          publicClient: wallet.publicClient,
          walletClient: wallet.walletClient,
          address: wallet.address,
          chainId: wallet.chainId,
        },
        tripleTermId: binding.triple_term_id as `0x${string}`,
        action: pendingAction,
        assets,
      });
      setPreview(next);
      setPhase("idle");
    } catch (err) {
      setPhase("failed");
      setMessage(err instanceof Error ? err.message : t.signalFailed);
    }
  }

  async function confirmDeposit() {
    if (!preview || !pendingAction || !wallet.publicClient || !wallet.walletClient || !wallet.address) {
      return;
    }
    setPhase("awaiting_signature");
    setMessage(null);
    try {
      const hash = await depositClaimSignal({
        clients: {
          publicClient: wallet.publicClient,
          walletClient: wallet.walletClient,
          address: wallet.address,
          chainId: wallet.chainId,
        },
        preview,
      });
      setPhase("submitted");
      setPhase("confirming");
      const receipt = await wallet.publicClient.waitForTransactionReceipt({
        hash,
        confirmations: 1,
      });
      if (receipt.status !== "success") {
        setPhase("failed");
        setMessage(t.signalFailed);
        return;
      }
      try {
        await syncClaimSignal(claimId, pendingAction, hash);
        setPhase("confirmed");
        setPreview(null);
        setPendingAction(null);
        onChanged?.();
      } catch (err) {
        setPhase("failed");
        setSyncRetryHash(hash);
        setMessage(err instanceof Error ? err.message : t.signalSyncRetry);
      }
    } catch (err) {
      setPhase("failed");
      const text = err instanceof Error ? err.message : t.signalFailed;
      if (/denied|rejected|user rejected/i.test(text)) {
        setMessage(t.signalRejected);
        setPreview(null);
        setPendingAction(null);
        return;
      }
      setMessage(text);
    }
  }

  async function retrySync() {
    if (!syncRetryHash || !pendingAction) return;
    try {
      await syncClaimSignal(claimId, pendingAction, syncRetryHash);
      setPhase("confirmed");
      setSyncRetryHash(null);
      setPreview(null);
      setPendingAction(null);
      onChanged?.();
    } catch (err) {
      setMessage(err instanceof Error ? err.message : t.signalSyncRetry);
    }
  }

  const busy = phase === "preparing" || phase === "awaiting_signature" || phase === "confirming" || phase === "submitted";
  const mineAction = (action: InteractionAction) => mine.includes(action);

  return (
    <div className="claim-action-bar">
      <p className="claim-action-bar__counts">
        {countOf(summary, "support")} {t.support} · {countOf(summary, "dispute")} {t.dispute} ·{" "}
        {countOf(summary, "uncertain")} {t.uncertain} · {t.comments(summary?.comment_count ?? 0)}
      </p>
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          className={`intuition-stance-btn intuition-stance-btn--support${mineAction("support") ? " is-mine" : ""}`}
          disabled={busy}
          onClick={() => void startEconomic("support")}
        >
          {t.support}
        </button>
        <button
          type="button"
          className={`intuition-stance-btn intuition-stance-btn--dispute${mineAction("dispute") ? " is-mine" : ""}`}
          disabled={busy}
          onClick={() => void startEconomic("dispute")}
        >
          {t.dispute}
        </button>
        <button
          type="button"
          className={`intuition-stance-btn intuition-stance-btn--uncertain${mineAction("uncertain") ? " is-mine" : ""}`}
          disabled={busy}
          onClick={() => void onUncertain()}
        >
          {t.uncertain}
        </button>
      </div>
      {intuitionStatusLabel ? (
        <p className="claim-action-bar__intuition">{intuitionStatusLabel}</p>
      ) : null}
      <p className="mt-1.5 text-[10px] leading-relaxed text-(--color-text-muted)">
        {t.signalEconomicHint}
      </p>
      {wallet.isConnected && !wallet.isSupportedChain ? (
        <button
          type="button"
          className="mt-2 intuition-stance-btn"
          onClick={() => switchChain({ chainId: requiredChain.id })}
        >
          {t.walletSwitchNetwork}
        </button>
      ) : null}
      {pendingAction ? (
        <div className="claim-action-bar__confirm">
          <p className="claim-action-bar__confirm-title">{t.signalConfirmTitle}</p>
          <p className="text-[11px] text-(--color-text-secondary)">{claimText}</p>
          <p className="mt-1 text-[11px]">
            {pendingAction === "support" ? t.support : t.dispute} · {requiredChain.name}
            {preview ? ` · ${formatTrustAmount(preview.assets)} TRUST` : null}
          </p>
          <div className="mt-2 flex flex-wrap gap-1.5">
            {TRUST_PRESETS.map((preset) => (
              <button
                key={preset}
                type="button"
                className={`agora-chip${amount === preset ? " is-active" : ""}`}
                onClick={() => {
                  setAmount(preset);
                  setPreview(null);
                }}
              >
                {preset} TRUST
              </button>
            ))}
            <button
              type="button"
              className={`agora-chip${amount === "custom" ? " is-active" : ""}`}
              onClick={() => {
                setAmount("custom");
                setPreview(null);
              }}
            >
              {t.signalCustomAmount}
            </button>
          </div>
          {amount === "custom" ? (
            <input
              className="mt-2 w-full rounded border border-(--color-border-subtle) bg-transparent px-2 py-1 text-sm"
              value={custom}
              onChange={(e) => {
                setCustom(e.target.value);
                setPreview(null);
              }}
              aria-label={t.signalCustomAmount}
            />
          ) : null}
          <div className="mt-2 flex gap-2">
            {!preview ? (
              <button
                type="button"
                className="intuition-stance-btn intuition-stance-btn--support"
                disabled={busy}
                onClick={() => void runPreview()}
              >
                {t.signalPreview}
              </button>
            ) : (
              <button
                type="button"
                className="intuition-stance-btn intuition-stance-btn--support"
                disabled={busy}
                onClick={() => void confirmDeposit()}
              >
                {t.signalConfirm}
              </button>
            )}
            <button
              type="button"
              className="intuition-stance-btn"
              disabled={busy}
              onClick={() => {
                setPreview(null);
                setPendingAction(null);
                setPhase("idle");
              }}
            >
              {t.close}
            </button>
          </div>
        </div>
      ) : null}
      {phase !== "idle" ? (
        <p className="mt-1.5 text-[11px] text-(--color-text-secondary)" role="status">
          {phaseLabel(phase)}
        </p>
      ) : null}
      {message ? (
        <p className="mt-1.5 text-[11px] text-(--color-trust-low)" role="alert">
          {message}
        </p>
      ) : null}
      {syncRetryHash ? (
        <button type="button" className="mt-2 intuition-stance-btn" onClick={() => void retrySync()}>
          {t.signalSyncRetry}
        </button>
      ) : null}
    </div>
  );
}

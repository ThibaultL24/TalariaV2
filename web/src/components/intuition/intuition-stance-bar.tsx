// web/src/components/intuition/intuition-stance-bar.tsx
import { useEffect, useState } from "react";
import {
  fetchIntuitionSignals,
  simulateIntuitionStance,
  type IntuitionStanceResponse,
  type IntuitionTargetKind,
} from "@/lib/api";
import { useI18n } from "@/lib/i18n";

interface IntuitionStanceBarProps {
  targetKind: IntuitionTargetKind;
  targetId: string;
  /** Compact single-line layout for source rows. */
  compact?: boolean;
  /**
   * When false (default), defer the signals fetch until the user interacts.
   * Agora lists can mount hundreds of claim cards — eager fetch becomes N+1.
   */
  eager?: boolean;
}

export function IntuitionStanceBar({
  targetKind,
  targetId,
  compact = false,
  eager = false,
}: IntuitionStanceBarProps) {
  const { t } = useI18n();
  const [activated, setActivated] = useState(eager);
  const [signals, setSignals] = useState<IntuitionStanceResponse | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!activated) return;
    let cancelled = false;
    setMessage(null);
    fetchIntuitionSignals(targetKind, targetId)
      .then((payload) => {
        if (!cancelled) setSignals(payload);
      })
      .catch(() => {
        if (!cancelled) setSignals(null);
      });
    return () => {
      cancelled = true;
    };
  }, [activated, targetKind, targetId]);

  async function onStance(stance: "believe" | "dispute") {
    setActivated(true);
    setBusy(true);
    setMessage(null);
    try {
      const result = await simulateIntuitionStance(targetKind, targetId, stance);
      if (result.code === "ready" || result.code === "modeled") {
        if (result.code === "ready" && result.preview) {
          setMessage(t.stanceTestnetReady);
        } else if (result.code === "modeled") setMessage(t.stanceModeled);
        else setMessage(t.stanceTestnetReady);
      } else if (result.code === "not_on_chain") setMessage(t.stanceNotOnChain);
      else if (result.code === "live_disabled") setMessage(t.stanceLiveDisabled);
      else if (result.code === "not_eligible" || result.code === "not_theory")
        setMessage(t.stanceNotEligible);
      else setMessage(result.reason ?? t.stanceLiveDisabled);
    } catch {
      setMessage(t.stanceLiveDisabled);
    } finally {
      setBusy(false);
    }
  }

  const hint =
    message ??
    (!activated
      ? t.stanceHint
      : signals?.code === "ready"
        ? t.stanceTestnetReady
        : signals?.code === "modeled"
          ? signals.intuition_live_allowed
            ? t.stanceTestnetReady
            : t.stanceModeled
          : signals?.code === "not_on_chain"
            ? t.stanceNotOnChain
            : signals?.code === "live_disabled"
              ? t.stanceLiveDisabled
              : signals?.code === "not_eligible" || signals?.code === "not_theory"
                ? t.stanceNotEligible
                : t.stanceHint);

  return (
    <div
      className={
        compact
          ? "mt-1.5"
          : "mt-3 border-t border-(--color-border-subtle)/70 pt-2"
      }
      onMouseEnter={() => setActivated(true)}
      onFocusCapture={() => setActivated(true)}
    >
      {!compact ? (
        <p className="mb-1.5 text-[10px] font-semibold uppercase tracking-wide text-(--color-text-muted)">
          {t.intuitionStanceLabel}
        </p>
      ) : null}
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          disabled={busy}
          onClick={() => void onStance("believe")}
          className="rounded-md border border-emerald-500/40 bg-emerald-500/10 px-2.5 py-1 text-[11px] font-medium text-emerald-100 disabled:opacity-40"
        >
          {t.believe}
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={() => void onStance("dispute")}
          className="rounded-md border border-rose-500/40 bg-rose-500/10 px-2.5 py-1 text-[11px] font-medium text-rose-100 disabled:opacity-40"
        >
          {t.dispute}
        </button>
      </div>
      <p className="mt-1.5 text-[10px] leading-relaxed text-(--color-text-muted)">{hint}</p>
    </div>
  );
}

export function isStanceClaimKind(claimKind: string): boolean {
  const kind = claimKind.trim().toLowerCase();
  return kind === "theory" || kind === "controversy" || kind === "debate_stance";
}

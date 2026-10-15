// web/src/lib/intuition/format-balance.ts
import { formatUnits } from "viem";

export function formatTokenBalance(value: bigint, decimals: number, maxFractionDigits = 4): string {
  const raw = formatUnits(value, decimals);
  const n = Number.parseFloat(raw);
  if (!Number.isFinite(n)) return raw;
  return n.toFixed(maxFractionDigits);
}

// web/src/components/wallet/wallet-connect-button.tsx
import { IntuitionConnectButton } from "@/components/wallet/intuition-connect-button";

interface WalletConnectButtonProps {
  compact?: boolean;
}

export function WalletConnectButton({ compact = false }: WalletConnectButtonProps) {
  return <IntuitionConnectButton variant={compact ? "navbar" : "panel"} />;
}

// web/src/lib/intuition/wallet-options.ts
import type { Connector } from "wagmi";

export interface IntuitionWalletOption {
  id: string;
  name: string;
  connectorKeys: string[];
}

export const INTUITION_WALLET_OPTIONS: IntuitionWalletOption[] = [
  { id: "browser", name: "Browser Wallet", connectorKeys: ["injected", "browser wallet"] },
  { id: "rabby", name: "Rabby Wallet", connectorKeys: ["rabby", "rabby wallet"] },
  { id: "metaMask", name: "MetaMask", connectorKeys: ["metamask", "io.metamask"] },
  { id: "walletConnect", name: "WalletConnect", connectorKeys: ["walletconnect", "wallet connect"] },
  { id: "binance", name: "Binance Wallet", connectorKeys: ["binance", "bnb"] },
  { id: "okx", name: "OKX Wallet", connectorKeys: ["okx"] },
  { id: "bybit", name: "Bybit Wallet", connectorKeys: ["bybit"] },
  { id: "bitget", name: "Bitget Wallet", connectorKeys: ["bitget", "bitkeep"] },
];

export function resolveConnectorForOption(
  option: IntuitionWalletOption,
  connectors: readonly Connector[],
): Connector | undefined {
  const keys = option.connectorKeys.map((key) => key.toLowerCase());
  const direct = connectors.find((connector) => {
    const hay = `${connector.id} ${connector.name} ${connector.type}`.toLowerCase();
    return keys.some((key) => hay.includes(key));
  });
  if (direct) return direct;
  if (option.id === "walletConnect") return undefined;
  return connectors.find(
    (connector) =>
      connector.type === "injected" || connector.id.toLowerCase().includes("injected"),
  );
}

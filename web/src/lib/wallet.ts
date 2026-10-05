// web/src/lib/wallet.ts
export interface Eip1193Provider {
  request: (args: { method: string; params?: unknown[] }) => Promise<unknown>;
}

export function getEthereumProvider(): Eip1193Provider | undefined {
  if (typeof window === "undefined") return undefined;
  const eth = (window as Window & { ethereum?: Eip1193Provider }).ethereum;
  return eth;
}

export async function connectWallet(): Promise<string> {
  const provider = getEthereumProvider();
  if (!provider) {
    throw new Error("no_wallet_provider");
  }
  const accounts = (await provider.request({
    method: "eth_requestAccounts",
  })) as string[];
  const address = accounts[0]?.trim();
  if (!address) throw new Error("no_account");
  return address;
}

export function shortenAddress(address: string): string {
  if (address.length < 12) return address;
  return `${address.slice(0, 6)}…${address.slice(-4)}`;
}

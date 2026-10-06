// web/src/lib/intuition/config.ts
import {
  getMultiVaultAddressFromChainId,
  intuitionMainnet,
  intuitionTestnet,
} from "@0xintuition/sdk";
import { API_URL_DEV, API_URL_PROD } from "@0xintuition/graphql";
import type { Chain } from "viem";

export type IntuitionEnvironment = "testnet" | "mainnet";

export const INTUITION_TESTNET_CHAIN_ID = 13579;
export const INTUITION_MAINNET_CHAIN_ID = 1155;

export function isSupportedIntuitionChain(chainId: number): boolean {
  return chainId === INTUITION_TESTNET_CHAIN_ID || chainId === INTUITION_MAINNET_CHAIN_ID;
}

export function getIntuitionChain(env: IntuitionEnvironment = "testnet"): Chain {
  return env === "mainnet" ? intuitionMainnet : intuitionTestnet;
}

export function getIntuitionApiUrl(env: IntuitionEnvironment = "testnet"): string {
  return env === "mainnet" ? API_URL_PROD : API_URL_DEV;
}

export function intuitionEnvForChain(chainId: number): IntuitionEnvironment {
  if (chainId === INTUITION_TESTNET_CHAIN_ID) return "testnet";
  if (chainId === INTUITION_MAINNET_CHAIN_ID) return "mainnet";
  throw new Error(`unsupported intuition chain: ${chainId}`);
}

export function getIntuitionApiUrlForChain(chainId: number): string {
  return getIntuitionApiUrl(intuitionEnvForChain(chainId));
}

export function getIntuitionMultiVaultAddress(chainId: number): `0x${string}` {
  intuitionEnvForChain(chainId);
  return getMultiVaultAddressFromChainId(chainId);
}

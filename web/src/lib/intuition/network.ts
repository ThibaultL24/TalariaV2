// web/src/lib/intuition/network.ts
import type { IntuitionEnvironment } from "./config";
import {
  INTUITION_MAINNET_CHAIN_ID,
  INTUITION_TESTNET_CHAIN_ID,
  getIntuitionChain,
  isSupportedIntuitionChain,
} from "./config";

export {
  INTUITION_MAINNET_CHAIN_ID,
  INTUITION_TESTNET_CHAIN_ID,
  getIntuitionChain,
  isSupportedIntuitionChain,
};

export function defaultIntuitionEnvironment(): IntuitionEnvironment {
  return import.meta.env.VITE_INTUITION_ENV === "mainnet" ? "mainnet" : "testnet";
}

export function appIntuitionChain() {
  return getIntuitionChain(defaultIntuitionEnvironment());
}

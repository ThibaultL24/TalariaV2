// web/src/stores/wallet-store.ts
import { create } from "zustand";
import { persist } from "zustand/middleware";
import { connectWallet } from "@/lib/wallet";

interface WalletState {
  address: string | null;
  connecting: boolean;
  error: string | null;
  connect: () => Promise<void>;
  disconnect: () => void;
  clearError: () => void;
}

export const useWalletStore = create<WalletState>()(
  persist(
    (set) => ({
      address: null,
      connecting: false,
      error: null,
      connect: async () => {
        set({ connecting: true, error: null });
        try {
          const address = await connectWallet();
          set({ address, connecting: false });
        } catch (e) {
          const code = e instanceof Error ? e.message : "connect_failed";
          set({ connecting: false, error: code });
        }
      },
      disconnect: () => set({ address: null, error: null }),
      clearError: () => set({ error: null }),
    }),
    {
      name: "talaria-wallet-v1",
      partialize: (state) => ({ address: state.address }),
    },
  ),
);

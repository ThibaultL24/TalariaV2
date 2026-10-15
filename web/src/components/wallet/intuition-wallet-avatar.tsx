// web/src/components/wallet/intuition-wallet-avatar.tsx
interface IntuitionWalletAvatarProps {
  address: string;
  size?: number;
  className?: string;
}

export function IntuitionWalletAvatar({
  size = 28,
  className = "",
}: IntuitionWalletAvatarProps) {
  return (
    <span
      className={`intuition-wallet-avatar inline-block shrink-0 rounded-full ring-1 ring-white/15 ${className}`}
      style={{
        width: size,
        height: size,
        background:
          "linear-gradient(135deg, var(--color-accent) 0%, var(--color-accent-strong) 100%)",
      }}
      aria-hidden
    />
  );
}

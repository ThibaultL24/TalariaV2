// web/src/components/brand/talaria-wordmark.tsx
import { TalariaWingedTImage } from "@/components/brand/talaria-brand-images";

interface TalariaWordmarkProps {
  className?: string;
  variant?: "navbar" | "hero";
}

export function TalariaWordmark({ className = "", variant = "navbar" }: TalariaWordmarkProps) {
  const rootClass =
    variant === "hero"
      ? `talaria-wordmark talaria-wordmark--hero ${className}`.trim()
      : `talaria-wordmark talaria-wordmark--navbar ${className}`.trim();

  return (
    <span className={rootClass}>
      <TalariaWingedTImage className="talaria-wordmark__glyph-img" alt="" />
      <span className="talaria-wordmark__rest">alaria</span>
    </span>
  );
}

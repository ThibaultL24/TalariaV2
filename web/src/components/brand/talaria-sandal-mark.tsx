// web/src/components/brand/talaria-sandal-mark.tsx
import { BRAND_SANDAL } from "@/components/brand/brand-assets";

interface TalariaSandalMarkProps {
  className?: string;
}

export function TalariaSandalMark({ className = "" }: TalariaSandalMarkProps) {
  return (
    <img
      src={BRAND_SANDAL}
      alt=""
      className={`talaria-sandal-mark ${className}`.trim()}
      decoding="async"
      draggable={false}
    />
  );
}

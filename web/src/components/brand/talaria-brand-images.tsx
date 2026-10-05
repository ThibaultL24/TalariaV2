// web/src/components/brand/talaria-brand-images.tsx
import {
  BRAND_SANDAL,
  BRAND_WINGED_T_AEGEAN,
  BRAND_WINGED_T_NAVY,
} from "@/components/brand/brand-assets";
import { useThemeStore } from "@/stores/theme-store";

interface BrandImageProps {
  className?: string;
  alt?: string;
}

export function TalariaWingedTImage({ className, alt = "" }: BrandImageProps) {
  const theme = useThemeStore((s) => s.theme);
  const src = theme === "dark" ? BRAND_WINGED_T_NAVY : BRAND_WINGED_T_AEGEAN;

  return (
    <img
      src={src}
      alt={alt}
      className={className}
      decoding="async"
      draggable={false}
    />
  );
}

export function TalariaSandalImage({ className, alt = "" }: BrandImageProps) {
  return (
    <img
      src={BRAND_SANDAL}
      alt={alt}
      className={className}
      decoding="async"
      draggable={false}
    />
  );
}

// web/src/components/brand/talaria-brand-lockup.tsx
import { BRAND_WORDMARK_WINGED_T } from "@/components/brand/brand-assets";
import { useI18n } from "@/lib/i18n";

interface TalariaBrandLockupProps {
  variant: "navbar" | "hero";
}

/** Winged-T wordmark, recolored per theme. */
export function TalariaBrandLockup({ variant }: TalariaBrandLockupProps) {
  const { t } = useI18n();
  const wrapClass = `talaria-brand-lockup talaria-brand-lockup--${variant}`;

  return (
    <span className={wrapClass}>
      <img
        src={BRAND_WORDMARK_WINGED_T}
        alt={t.productName}
        className="talaria-brand-lockup__logo"
        decoding="async"
        draggable={false}
      />
    </span>
  );
}

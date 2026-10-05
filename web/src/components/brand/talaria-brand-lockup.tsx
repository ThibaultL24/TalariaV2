// web/src/components/brand/talaria-brand-lockup.tsx
import { BRAND_SANDAL } from "@/components/brand/brand-assets";
import { useI18n } from "@/lib/i18n";

interface TalariaBrandLockupProps {
  variant: "navbar" | "hero";
}

/** Standalone winged sandal (B&W, no plate) beside live type. */
export function TalariaBrandLockup({ variant }: TalariaBrandLockupProps) {
  const { t } = useI18n();
  const wrapClass = `talaria-brand-lockup talaria-brand-lockup--${variant}`;

  return (
    <span className={wrapClass}>
      <span className="talaria-brand-lockup__mark" aria-hidden>
        <img
          src={BRAND_SANDAL}
          alt=""
          className="talaria-brand-lockup__sandal"
          decoding="async"
          draggable={false}
        />
      </span>
      <span className="talaria-brand-lockup__type">
        <span className="talaria-brand-lockup__name">{t.productName}</span>
        <span className="talaria-brand-lockup__tagline">{t.productSubtitle}</span>
      </span>
    </span>
  );
}

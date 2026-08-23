// web/src/components/layout/navbar.tsx
import type { ReactNode } from "react";
import { useI18n } from "@/lib/i18n";

interface NavbarProps {
  center?: ReactNode;
}

export function Navbar({ center }: NavbarProps) {
  const { locale, setLocale, t } = useI18n();

  return (
    <header className="navbar">
      <a href="/" className="navbar__brand">
        <div>
          <div className="navbar__title">{t.productName}</div>
          <div className="navbar__subtitle">{t.productSubtitle}</div>
        </div>
      </a>
      {center ? <div className="mx-3 min-w-0 max-w-xl flex-1">{center}</div> : null}
      <div className="navbar__actions">
        <div className="flex overflow-hidden rounded-full border border-(--color-border-subtle) text-[11px] font-semibold">
          <button
            type="button"
            className={`px-2.5 py-1 ${locale === "fr" ? "bg-(--color-bg-surface) text-(--color-text-primary)" : "text-(--color-text-muted)"}`}
            onClick={() => setLocale("fr")}
            aria-pressed={locale === "fr"}
          >
            FR
          </button>
          <button
            type="button"
            className={`px-2.5 py-1 ${locale === "en" ? "bg-(--color-bg-surface) text-(--color-text-primary)" : "text-(--color-text-muted)"}`}
            onClick={() => setLocale("en")}
            aria-pressed={locale === "en"}
          >
            EN
          </button>
        </div>
      </div>
    </header>
  );
}

// web/src/components/layout/navbar.tsx
import type { ReactNode } from "react";
import { Link, NavLink } from "react-router-dom";
import { useI18n } from "@/lib/i18n";

interface NavbarProps {
  center?: ReactNode;
}

export function Navbar({ center }: NavbarProps) {
  const { locale, setLocale, t } = useI18n();

  const linkClass = ({ isActive }: { isActive: boolean }) =>
    `rounded-full px-2.5 py-1 text-[11px] font-semibold ${
      isActive
        ? "bg-(--color-bg-surface) text-(--color-text-primary)"
        : "text-(--color-text-muted) hover:text-(--color-text-primary)"
    }`;

  return (
    <header className="navbar">
      <Link to="/" className="navbar__brand">
        <div>
          <div className="navbar__title">{t.productName}</div>
          <div className="navbar__subtitle">{t.productSubtitle}</div>
        </div>
      </Link>
      {center ? <div className="mx-3 min-w-0 max-w-xl flex-1">{center}</div> : null}
      <div className="navbar__actions">
        <NavLink to="/" end className={linkClass}>
          {t.home}
        </NavLink>
        <NavLink to="/explorer" className={linkClass}>
          {t.explorer}
        </NavLink>
        <NavLink to="/agora" className={linkClass}>
          {t.agora}
        </NavLink>
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

// web/src/components/layout/navbar.tsx
import type { ReactNode } from "react";
import { Link, NavLink, useLocation } from "react-router-dom";
import { useI18n } from "@/lib/i18n";
import { TalariaNavbarBrand } from "@/components/brand/talaria-navbar-brand";
import { ThemeToggle } from "@/components/layout/theme-toggle";
import { IntuitionConnectButton } from "@/components/wallet/intuition-connect-button";
import { entityIdFromPathname, type ExperienceId } from "@/lib/experiences";
import { useExplorerStore } from "@/stores/explorer-store";

interface NavbarProps {
  center?: ReactNode;
}

export function Navbar({ center }: NavbarProps) {
  const { locale, setLocale, t } = useI18n();
  const { pathname } = useLocation();
  const storedId = useExplorerStore((state) => state.entityId);
  const entityId = entityIdFromPathname(pathname) ?? storedId;

  const linkClass = ({ isActive }: { isActive: boolean }) =>
    `rounded-full px-2.5 py-1 text-[11px] font-semibold ${
      isActive
        ? "bg-(--color-bg-surface) text-(--color-text-primary)"
        : "text-(--navbar-chrome-muted) hover:text-(--navbar-chrome-text)"
    }`;

  const experienceClass = (experience: ExperienceId) => {
    const active = pathname === `/${experience}` || pathname.startsWith(`/${experience}/`);
    return `rounded-full px-2.5 py-1 text-[11px] font-semibold ${
      active
        ? "bg-(--color-bg-surface) text-(--color-text-primary)"
        : "text-(--navbar-chrome-muted) hover:text-(--navbar-chrome-text)"
    }`;
  };

  return (
    <header className="navbar">
      <Link to="/" className="navbar__brand" aria-label={t.productName}>
        <TalariaNavbarBrand />
      </Link>
      {center ? <div className="navbar__center">{center}</div> : null}
      <div className="navbar__actions">
        <nav className="navbar__links" aria-label="Main">
          <NavLink to="/" end className={linkClass}>
            {t.home}
          </NavLink>
          <Link
            to={entityId ? `/istoria/${encodeURIComponent(entityId)}` : "/istoria"}
            className={experienceClass("istoria")}
            aria-current={pathname === "/istoria" || pathname.startsWith("/istoria/") ? "page" : undefined}
          >
            Istoria
          </Link>
          <Link
            to={entityId ? `/poreia/${encodeURIComponent(entityId)}` : "/poreia"}
            className={experienceClass("poreia")}
            aria-current={pathname === "/poreia" || pathname.startsWith("/poreia/") ? "page" : undefined}
          >
            Poreia
          </Link>
          <Link
            to={entityId ? `/agora/${encodeURIComponent(entityId)}` : "/agora"}
            className={experienceClass("agora")}
            aria-current={pathname === "/agora" || pathname.startsWith("/agora/") ? "page" : undefined}
          >
            {t.agora}
          </Link>
          <NavLink to="/about" className={linkClass}>
            {t.whitepaperNav}
          </NavLink>
        </nav>
        <IntuitionConnectButton variant="navbar" />
        <ThemeToggle />
        <div className="flex shrink-0 overflow-hidden rounded-full border border-(--color-border-subtle) text-[11px] font-semibold">
          <button
            type="button"
            className={`px-2.5 py-1 ${locale === "fr" ? "bg-(--color-bg-surface) text-(--color-text-primary)" : "text-(--navbar-chrome-muted)"}`}
            onClick={() => setLocale("fr")}
            aria-pressed={locale === "fr"}
          >
            FR
          </button>
          <button
            type="button"
            className={`px-2.5 py-1 ${locale === "en" ? "bg-(--color-bg-surface) text-(--color-text-primary)" : "text-(--navbar-chrome-muted)"}`}
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

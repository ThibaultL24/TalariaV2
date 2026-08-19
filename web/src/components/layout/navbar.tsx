// web/src/components/layout/navbar.tsx
import type { StatusResponse } from "@/lib/api";

interface NavbarProps {
  status?: StatusResponse | null;
}

export function Navbar({ status }: NavbarProps) {
  const events = status?.counts.canonical_events ?? 0;

  return (
    <header className="navbar">
      <a href="/" className="navbar__brand">
        <div>
          <div className="navbar__title">Talaria</div>
          <div className="navbar__subtitle">Engine Explorer</div>
        </div>
      </a>
      <div className="navbar__actions">
        {status ? (
          <span className="navbar__stat">
            {events} canonical events
            <span className="ml-2 text-(--color-text-muted)">live 5s</span>
          </span>
        ) : null}
      </div>
    </header>
  );
}

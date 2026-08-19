// web/src/components/layout/navbar.tsx
import type { StatusResponse } from "@/lib/api";
import { strings } from "@/lib/strings";

interface NavbarProps {
  status?: StatusResponse | null;
}

export function Navbar({ status }: NavbarProps) {
  const mapped = status?.counts.canonical_events ?? 0;

  return (
    <header className="navbar">
      <a href="/" className="navbar__brand">
        <div>
          <div className="navbar__title">{strings.productName}</div>
          <div className="navbar__subtitle">{strings.productSubtitle}</div>
        </div>
      </a>
      <div className="navbar__actions">
        {status ? (
          <span className="navbar__stat">{mapped.toLocaleString()} events in library</span>
        ) : null}
      </div>
    </header>
  );
}

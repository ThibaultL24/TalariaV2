// web/src/app.tsx
import { BrowserRouter, Navigate, Route, Routes, useLocation, useParams } from "react-router-dom";
import { AgoraPage } from "@/features/agora/pages/agora-page";
import { IstoriaPage } from "@/features/istoria/pages/istoria-page";
import { PoreiaPage } from "@/features/poreia/pages/poreia-page";
import { HomePage } from "@/pages/home-page";
import { IngestionAdminPage } from "@/features/admin/pages/ingestion-admin-page";
import { legacyEntityTarget } from "@/lib/experiences";

import { WhitepaperPage } from "@/pages/whitepaper-page";

export function LegacyEntityRedirect() {
  const { entityId = "", view } = useParams();
  const { search } = useLocation();
  return <Navigate to={legacyEntityTarget(entityId, view, search)} replace />;
}

export function App() {
  return (
    <>
      <div className="talaria-ambient" aria-hidden="true">
        <div className="talaria-ambient__layer talaria-ambient__sea" />
        <div className="talaria-ambient__layer talaria-ambient__grain" />
      </div>
      <BrowserRouter>
      <Routes>
        <Route path="/" element={<HomePage />} />
        <Route path="/istoria" element={<IstoriaPage />} />
        <Route path="/istoria/:entityId" element={<IstoriaPage />} />
        <Route path="/istoria/:entityId/:view" element={<IstoriaPage />} />
        <Route path="/poreia" element={<PoreiaPage />} />
        <Route path="/poreia/:entityId" element={<PoreiaPage />} />
        <Route path="/poreia/:entityId/:view" element={<PoreiaPage />} />
        <Route path="/agora" element={<AgoraPage />} />
        <Route path="/agora/:entityId" element={<AgoraPage />} />
        <Route path="/agora/:entityId/:view" element={<AgoraPage />} />
        <Route path="/entities/:entityId" element={<LegacyEntityRedirect />} />
        <Route path="/entities/:entityId/:view" element={<LegacyEntityRedirect />} />
        <Route path="/explorer" element={<Navigate to="/istoria" replace />} />
        <Route path="/admin/ingestions" element={<IngestionAdminPage />} />
        <Route path="/about" element={<WhitepaperPage />} />
        <Route path="/whitepaper" element={<Navigate to="/about" replace />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
    </>
  );
}

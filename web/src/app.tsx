// web/src/app.tsx
import { EntityPage } from "@/features/entity/entity-page";
import { BrowserRouter, Navigate, Route, Routes, useLocation, useParams } from "react-router-dom";
import { AgoraPage } from "@/pages/agora-page";
import { HomePage } from "@/pages/home-page";

import { WhitepaperPage } from "@/pages/whitepaper-page";

function LegacyEntityRedirect() {
  const { entityId, view } = useParams();
  const { search } = useLocation();
  const params = new URLSearchParams(search);
  const experience = params.get("lens") === "visit" ? "poreia" : "istoria";
  params.delete("lens");
  const valid = experience === "poreia" ? ["map", "heritage", "now"] : ["overview", "timeline", "map", "sources"];
  const nextView = view && valid.includes(view) ? view : experience === "poreia" ? "map" : "overview";
  return <Navigate to={`/${experience}/${entityId}/${nextView}${params.size ? `?${params}` : ""}`} replace />;
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
        <Route path="/istoria" element={<EntityPage experience="istoria" />} />
        <Route path="/istoria/:entityId" element={<EntityPage experience="istoria" />} />
        <Route path="/istoria/:entityId/:view" element={<EntityPage experience="istoria" />} />
        <Route path="/poreia" element={<EntityPage experience="poreia" />} />
        <Route path="/poreia/:entityId" element={<EntityPage experience="poreia" />} />
        <Route path="/poreia/:entityId/:view" element={<EntityPage experience="poreia" />} />
        <Route path="/entities/:entityId" element={<LegacyEntityRedirect />} />
        <Route path="/entities/:entityId/:view" element={<LegacyEntityRedirect />} />
        <Route path="/explorer" element={<Navigate to="/istoria" replace />} />
        <Route path="/agora" element={<AgoraPage />} />
        <Route path="/about" element={<WhitepaperPage />} />
        <Route path="/whitepaper" element={<Navigate to="/about" replace />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
    </>
  );
}

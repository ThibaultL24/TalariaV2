// web/src/app.tsx
import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { AgoraPage } from "@/pages/agora-page";
import { ExplorerPage } from "@/pages/explorer-page";
import { HomePage } from "@/pages/home-page";

import { WhitepaperPage } from "@/pages/whitepaper-page";

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<HomePage />} />
        <Route path="/explorer" element={<ExplorerPage />} />
        <Route path="/agora" element={<AgoraPage />} />
        <Route path="/about" element={<WhitepaperPage />} />
        <Route path="/whitepaper" element={<Navigate to="/about" replace />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  );
}

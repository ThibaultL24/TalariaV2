// web/src/main.tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { ExplorerPage } from "./pages/explorer-page";
import "./index.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ExplorerPage />
  </StrictMode>,
);

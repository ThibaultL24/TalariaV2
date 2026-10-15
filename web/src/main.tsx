// web/src/main.tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app";
import { IntuitionProviders } from "@/lib/intuition/providers";
import "./index.css";

import { initializeTheme } from "./stores/theme-store";
initializeTheme();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <IntuitionProviders>
      <App />
    </IntuitionProviders>
  </StrictMode>,
);

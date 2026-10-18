// web/src/features/entity/entity-page.tsx
import { PoreiaPage } from "@/features/poreia/pages/poreia-page";
import { IstoriaPage } from "@/features/istoria/pages/istoria-page";

export { databaseTabs } from "./database-tabs";

/** Compatibility mount for tests and any caller that still picks the experience by prop. */
export function EntityPage({ experience = "istoria" }: { experience?: "istoria" | "poreia" }) {
  return experience === "poreia" ? <PoreiaPage /> : <IstoriaPage />;
}

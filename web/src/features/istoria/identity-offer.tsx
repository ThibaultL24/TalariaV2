// web/src/features/istoria/identity-offer.tsx
import type { SearchSuggestion } from "@/lib/schemas/entity";
import { researchDecision } from "@/lib/search-suggestions";

export function IdentityOffer({
  identity,
  locale,
  busy,
  error,
  onResearch,
}: {
  identity: SearchSuggestion;
  locale: "fr" | "en";
  busy: boolean;
  error?: string | null;
  onResearch: () => void;
}) {
  const decision = researchDecision(identity);
  const fr = locale === "fr";
  if (decision === "open") return null;
  return (
    <section className="v3-note" aria-label={fr ? "Identité" : "Identity"}>
      <h2>{identity.label}</h2>
      {identity.qid && (
        <p>{fr ? `Identifiant retenu : ${identity.qid}` : `Selected identifier: ${identity.qid}`}</p>
      )}
      {identity.description && <p>{identity.description}</p>}
      {decision === "unresolved" && (
        <p>
          {fr
            ? "Aucun identifiant Wikidata fiable. Aucune fiche n'est créée."
            : "No reliable Wikidata identifier. No record is created."}
        </p>
      )}
      {decision === "offer" && (
        <>
          <p>
            {fr
              ? "Cette personnalité n'est pas encore dans Talaria. La recherche documentaire ne démarre que sur demande."
              : "This person is not in Talaria yet. Documentary research starts only when you ask."}
          </p>
          <button type="button" onClick={onResearch} disabled={busy}>
            {fr ? "Lancer la recherche" : "Start research"}
          </button>
        </>
      )}
      {error && <p role="alert">{error}</p>}
    </section>
  );
}

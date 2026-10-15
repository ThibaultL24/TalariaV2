import { useEffect, useState } from "react";
import { fetchPoreiaItinerary, type PoreiaItinerary } from "@/lib/entity-views";

export function PoreiaItineraryView({
  entityId,
  onFocus,
}: {
  entityId: string;
  onFocus: (point: { id: string; lat: number; lon: number }) => void;
}) {
  const [result, setResult] = useState<PoreiaItinerary>();
  const [error, setError] = useState("");
  const [destination, setDestination] = useState<{lat:number;lon:number;radiusKm:number}|undefined>();
  const [latInput, setLatInput] = useState("");
  const [lonInput, setLonInput] = useState("");
  useEffect(() => {
    const controller = new AbortController();
    setResult(undefined);
    setError("");
    void fetchPoreiaItinerary(entityId, controller.signal, destination)
      .then(data => { if (!controller.signal.aborted) setResult(data); })
      .catch(e => { if (!controller.signal.aborted) setError(String(e)); });
    return () => controller.abort();
  }, [entityId, destination]);

  return (
    <section className="poreia-route" aria-labelledby="poreia-route-title">
      <header className="poreia-route__header">
        <p className="v3-eyebrow">POREIA · PARCOURS</p>
        <h2 id="poreia-route-title">Sur les traces de cette personnalité</h2>
        <p>Une proposition pour découvrir des sites réellement identifiés et liés à la personne.</p>
      </header>
      <form className="poreia-destination-form" onSubmit={(e) => {
        e.preventDefault();
        const lat = Number(latInput), lon = Number(lonInput);
        if (!latInput.trim() || !lonInput.trim() || !Number.isFinite(lat) || !Number.isFinite(lon) ||
            Math.abs(lat) > 90 || Math.abs(lon) > 180) {
          setError("Indiquez des coordonnées valides (latitude et longitude)."); return;
        }
        setDestination({lat, lon, radiusKm: 25});
      }}>
        <label>Latitude <input type="number" step="any" min="-90" max="90" required value={latInput} onChange={e => setLatInput(e.target.value)} placeholder="48.8566" /></label>
        <label>Longitude <input type="number" step="any" min="-180" max="180" required value={lonInput} onChange={e => setLonInput(e.target.value)} placeholder="2.3522" /></label>
        <button className="button button--primary" type="submit">Découvrir à proximité (25 km)</button>
        {destination && <button type="button" className="button button--ghost" onClick={() => setDestination(undefined)}>Tous les sites</button>}
      </form>
      <p className="v3-note">Filtrer autour d'une destination par ses coordonnées ; aucun lieu n'est inventé par proximité.</p>
      {error && <p role="alert">{error}</p>}
      {!result && !error && <p role="status">Chargement du parcours…</p>}
      {result?.message && <p>{result.message}</p>}
      {result?.stops.length ? (
        <ol className="poreia-route__stops">
          {result.stops.map((stop, index) => (
            <li key={stop.id} className="poreia-route__stop">
              <span className="poreia-route__index">{String(index + 1).padStart(2, "0")}</span>
              <div>
                <h3>{stop.label}</h3>
                <p>Site documenté · {stop.wikidata_qid}</p>
                {index > 0 && result.legs[index - 1] && (
                  <p>{result.legs[index - 1].straight_line_km.toLocaleString("fr-FR")} km à vol d’oiseau depuis le site précédent</p>
                )}
              </div>
              <button type="button" className="button button--ghost"
                onClick={() => onFocus({ id: stop.id, lat: stop.coordinates.lat, lon: stop.coordinates.lon })}>
                Voir sur la carte
              </button>
            </li>
          ))}
        </ol>
      ) : null}
      {result?.stops.length ? (
        <p className="poreia-route__disclaimer">
          {result.warning ?? "Parcours indicatif : les déplacements, l’ouverture et les horaires sont à vérifier."}
        </p>
      ) : null}
    </section>
  );
}

import { useEffect, useState } from "react";
import { fetchPoreiaItinerary, findPoreiaDestinations, type PoreiaDestination, type PoreiaItinerary } from "@/lib/entity-views";

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
  const [query, setQuery] = useState("");
  const [matches, setMatches] = useState<PoreiaDestination[]>([]);
  const [searching, setSearching] = useState(false);
  const [selectedCity, setSelectedCity] = useState<PoreiaDestination>();
  const [mode, setMode] = useState<"walking"|"cycling"|"driving">("walking");
  const [radiusKm, setRadiusKm] = useState(25);
  useEffect(() => {
    const controller = new AbortController();
    setResult(undefined);
    setError("");
    void fetchPoreiaItinerary(entityId, controller.signal, destination, mode)
      .then(data => { if (!controller.signal.aborted) setResult(data); })
      .catch(e => { if (!controller.signal.aborted) setError(String(e)); });
    return () => controller.abort();
  }, [entityId, destination, mode]);

  return (
    <section className="poreia-route" aria-labelledby="poreia-route-title">
      <header className="poreia-route__header">
        <p className="v3-eyebrow">POREIA · PARCOURS</p>
        <h2 id="poreia-route-title">Sur les traces de cette personnalité</h2>
        <p>Une proposition pour découvrir des sites réellement identifiés et liés à la personne.</p>
      </header>
      <form className="poreia-destination-form" onSubmit={(event) => {
        event.preventDefault();
        if (query.trim().length < 3) {setError("Saisissez au moins trois caractères.");return;}
        const controller = new AbortController();
        setSearching(true);setError("");setMatches([]);
        void findPoreiaDestinations(query,"fr",controller.signal)
          .then(setMatches)
          .catch(() => setError("Recherche indisponible. Réessayez ultérieurement."))
          .finally(() => setSearching(false));
      }}>
        <label>Destination
          <input type="search" value={query} maxLength={80}
            onChange={event => {setQuery(event.target.value);setMatches([]);}}
            placeholder="Paris, Avignon, Florence…" aria-label="Rechercher une ville" />
        </label>
        <button type="submit" className="button button--primary" disabled={searching}>
          {searching ? "Recherche…" : "Rechercher"}
        </button>
        <label>Transport
          <select value={mode} onChange={event => setMode(event.target.value as typeof mode)}>
            <option value="walking">À pied</option>
            <option value="cycling">À vélo</option>
            <option value="driving">En voiture</option>
          </select>
        </label>
        <label>Rayon
          <select value={radiusKm} onChange={event => {
            const value=Number(event.target.value);
            setRadiusKm(value);
            if(selectedCity) setDestination({...selectedCity.coordinates,radiusKm:value});
          }}>
            <option value={5}>5 km</option>
            <option value={10}>10 km</option>
            <option value={25}>25 km</option>
            <option value={50}>50 km</option>
          </select>
        </label>
      </form>
      {matches.length>0 && (
        <ul className="poreia-destination-results" aria-label="Villes trouvées">
          {matches.map(city => <li key={city.qid}>
            <button type="button" onClick={() => {
              setSelectedCity(city);
              setDestination({...city.coordinates,radiusKm});
              setQuery(city.label);setMatches([]);
            }}>
              <strong>{city.label}</strong><span>{city.description}</span>
            </button>
          </li>)}
        </ul>
      )}
      {selectedCity && (
        <p className="poreia-destination-selected">
          Destination : <strong>{selectedCity.label}</strong> ·
          <a href={selectedCity.source_url} target="_blank" rel="noopener noreferrer">source Wikidata ↗</a>
          <button type="button" onClick={() => {setSelectedCity(undefined);setDestination(undefined);}}>Effacer</button>
        </p>
      )}
      <p className="v3-note">Seuls les sites historiquement associés à la personne sont proposés. La recherche ne crée aucun lieu.</p>
      {result?.routing === "osrm" ? (
        <p className="poreia-route__metrics">
          Trajet calculé : {result.route_distance_km?.toLocaleString("fr-FR",{maximumFractionDigits:1})} km
          · environ {result.travel_minutes} min · {mode === "walking" ? "à pied" : mode === "cycling" ? "à vélo" : "en voiture"}
        </p>
      ) : result?.stops.length ? (
        <p className="v3-note">Aucun moteur de routage vérifié n'est configuré pour ce mode : seules les distances à vol d'oiseau sont disponibles.</p>
      ) : null}
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

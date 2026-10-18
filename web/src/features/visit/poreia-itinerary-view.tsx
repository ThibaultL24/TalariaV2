import { useEffect, useState } from "react";
import { fetchPoreiaItinerary, findPoreiaDestinations, type PoreiaDestination, type PoreiaItinerary } from "@/lib/entity-views";
import { useI18n } from "@/lib/i18n";

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
  const [radiusKm, setRadiusKm] = useState(25);
  const { t, locale } = useI18n();
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
        <div className="poreia-route__banner" role="img" aria-label={t.itineraryBanner} />
        <p className="v3-eyebrow">{t.itineraryKicker}</p>
        <h2 id="poreia-route-title">{t.itineraryTitle}</h2>
        <p>{t.itineraryIntro}</p>
      </header>
      <form className="poreia-destination-form" onSubmit={(event) => {
        event.preventDefault();
        if (query.trim().length < 3) {setError(t.needThreeCharacters);return;}
        const controller = new AbortController();
        setSearching(true);setError("");setMatches([]);
        void findPoreiaDestinations(query, locale, controller.signal)
          .then(setMatches)
          .catch(() => setError(t.searchUnavailableLater))
          .finally(() => setSearching(false));
      }}>
        <label>{t.destination}
          <input type="search" value={query} maxLength={80}
            onChange={event => {setQuery(event.target.value);setMatches([]);}}
            placeholder={t.cityPlaceholder} aria-label={t.searchCity} />
        </label>
        <button type="submit" className="button button--primary" disabled={searching}>
          {searching ? t.searching : t.search}
        </button>
        <label>{t.radius}
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
        <ul className="poreia-destination-results" aria-label={t.citiesFound}>
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
          {t.destination} : <strong>{selectedCity.label}</strong>
          <a href={selectedCity.source_url} target="_blank" rel="noopener noreferrer">{t.associationSource}</a>
          <button type="button" onClick={() => {setSelectedCity(undefined);setDestination(undefined);}}>{t.clearDestination}</button>
        </p>
      )}
      <p className="v3-note">{t.onlyAssociatedSites}</p>
      {result?.routing === "osrm" && result.travel_minutes != null ? (
        <p className="poreia-route__metrics">
          {t.calculatedRoute(result.route_distance_km?.toLocaleString(locale === "fr" ? "fr-FR" : "en-GB", { maximumFractionDigits: 1 }) ?? "", result.travel_minutes)}
        </p>
      ) : result?.stops.length ? (
        <p className="v3-note">{t.geoSequence}</p>
      ) : null}
      {error && <p role="alert">{error}</p>}
      {!result && !error && <p role="status">{t.loadingItinerary}</p>}
      {result?.message && <p>{result.message}</p>}
      {result?.stops.length ? (
        <ol className="poreia-route__stops">
          {result.stops.map((stop, index) => (
            <li key={stop.id} className="poreia-route__stop">
              <span className="poreia-route__index">{String(index + 1).padStart(2, "0")}</span>
              <div>
                <h3>{stop.label}</h3>
                {index > 0 && result.legs[index - 1] && (
                  <p>{t.straightLineFromPrevious(result.legs[index - 1].straight_line_km.toLocaleString(locale === "fr" ? "fr-FR" : "en-GB"))}</p>
                )}
              </div>
              <button type="button" className="button button--ghost"
                onClick={() => onFocus({ id: stop.id, lat: stop.coordinates.lat, lon: stop.coordinates.lon })}>
                {t.seeOnMap}
              </button>
            </li>
          ))}
        </ol>
      ) : null}
      {result?.stops.length ? (
        <p className="poreia-route__disclaimer">
          {result.warning ?? t.indicativeRoute}
        </p>
      ) : null}
    </section>
  );
}

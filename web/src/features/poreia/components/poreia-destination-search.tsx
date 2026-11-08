// web/src/features/poreia/components/poreia-destination-search.tsx
import { useState } from "react";
import { findPoreiaDestinations, type PoreiaDestination } from "@/lib/entity-views";
import { useI18n } from "@/lib/i18n";

export function PoreiaDestinationSearch({
  onSelect,
}: {
  onSelect: (city: PoreiaDestination, radiusKm: number) => void;
}) {
  const { locale, t } = useI18n();
  const [query, setQuery] = useState("");
  const [radiusKm, setRadiusKm] = useState(10);
  const [matches, setMatches] = useState<PoreiaDestination[]>([]);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState("");

  return (
    <form
      className="poreia-destination-form"
      onSubmit={(event) => {
        event.preventDefault();
        if (query.trim().length < 3) {
          setError(t.needThreeCharacters);
          return;
        }
        setSearching(true);
        setError("");
        void findPoreiaDestinations(query, locale)
          .then(setMatches)
          .catch(() => setError(t.searchUnavailableLater))
          .finally(() => setSearching(false));
      }}
    >
      <label>
        {t.destination}
        <input
          type="search"
          value={query}
          maxLength={80}
          onChange={(event) => {
            setQuery(event.target.value);
            setMatches([]);
          }}
          placeholder={t.cityPlaceholder}
          aria-label={t.searchCity}
        />
      </label>
      <label>
        {t.radius}
        <select value={radiusKm} onChange={(event) => setRadiusKm(Number(event.target.value))} aria-label={t.radius}>
          <option value={5}>5 km</option>
          <option value={10}>10 km</option>
          <option value={25}>25 km</option>
          <option value={50}>50 km</option>
        </select>
      </label>
      <button type="submit" className="button button--primary" disabled={searching}>
        {searching ? t.searching : t.search}
      </button>
      {error ? <p role="alert">{error}</p> : null}
      {matches.length > 0 ? (
        <ul className="poreia-destination-results" aria-label={t.citiesFound}>
          {matches.map((city) => (
            <li key={city.qid}>
              <button
                type="button"
                onClick={() => {
                  onSelect(city, radiusKm);
                  setQuery(city.label);
                  setMatches([]);
                }}
              >
                <strong>{city.label}</strong>
                <span>{city.description}</span>
              </button>
            </li>
          ))}
        </ul>
      ) : null}
    </form>
  );
}

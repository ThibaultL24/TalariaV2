// web/src/features/istoria/life-reader.tsx
import type { TimelineEvent } from "@/lib/api";
import { readableLabel } from "@/lib/human-label";
import type { LifeBand, LifeSpeed } from "./life-reading";
import { eventYear } from "./life-reading";

const SPEEDS: LifeSpeed[] = [0.5, 1, 2, 4];

interface LifeReaderProps {
  locale: "fr" | "en";
  playing: boolean;
  showAll: boolean;
  speed: LifeSpeed;
  bands: LifeBand[];
  band: LifeBand | null;
  index: number;
  total: number;
  undated: number;
  active?: TimelineEvent;
  onPlay: () => void;
  onPause: () => void;
  onStep: (delta: number) => void;
  onRestart: () => void;
  onSpeed: (speed: LifeSpeed) => void;
  onShowAll: () => void;
  onBand: (band: LifeBand | null) => void;
}

export function LifeReader({
  locale,
  playing,
  showAll,
  speed,
  bands,
  band,
  index,
  total,
  undated,
  active,
  onPlay,
  onPause,
  onStep,
  onRestart,
  onSpeed,
  onShowAll,
  onBand,
}: LifeReaderProps) {
  const fr = locale === "fr";
  const year = active ? eventYear(active) : null;
  const when = active?.time?.surface
    ?? (year !== null ? String(year) : (fr ? "Date non précisée" : "Date unstated"));

  return (
    <section
      className="life-reader"
      aria-label={fr ? "Lecture de la vie" : "Life reading"}
      onKeyDown={(event) => {
        if (event.key === "ArrowRight") onStep(1);
        if (event.key === "ArrowLeft") onStep(-1);
      }}
    >
      <div className="life-reader__controls">
        <button type="button" className="life-reader__play" onClick={playing ? onPause : onPlay}>
          {playing ? (fr ? "Pause" : "Pause") : (fr ? "Lire sa vie" : "Read this life")}
        </button>
        <button type="button" onClick={onRestart}>{fr ? "Début" : "Start"}</button>
        <button type="button" aria-label={fr ? "Événement précédent" : "Previous event"} onClick={() => onStep(-1)}>←</button>
        <button type="button" aria-label={fr ? "Événement suivant" : "Next event"} onClick={() => onStep(1)}>→</button>
        <label>
          {fr ? "Vitesse" : "Speed"}
          <select
            value={String(speed)}
            onChange={(event) => onSpeed(Number(event.target.value) as LifeSpeed)}
          >
            {SPEEDS.map((item) => (
              <option key={item} value={item}>{item}×</option>
            ))}
          </select>
        </label>
        <label>
          {fr ? "Période" : "Period"}
          <select
            value={band?.id ?? ""}
            onChange={(event) => onBand(bands.find((item) => item.id === event.target.value) ?? null)}
          >
            <option value="">{fr ? "Toute la vie" : "Whole life"}</option>
            {bands.map((item) => (
              <option key={item.id} value={item.id}>{item.label}</option>
            ))}
          </select>
        </label>
        <button type="button" aria-pressed={showAll} onClick={onShowAll}>
          {fr ? "Tous les événements" : "All events"}
        </button>
      </div>
      <p className="life-reader__note">
        {fr
          ? "Le trait relie les lieux dans l’ordre des faits datés. Ce n’est pas un itinéraire attesté. Le fond est une carte actuelle : ses frontières ne sont pas celles de l’époque."
          : "The line follows dated facts in time. It is not a documented journey. The base map is current: its borders are not those of the period."}
        {undated > 0
          ? fr
            ? ` ${undated} faits sans date restent hors de la lecture.`
            : ` ${undated} undated facts stay outside the reading.`
          : ""}
      </p>
      {active && !showAll ? (
        <article className="life-reader__card">
          <p>{when}{active.place_label ? ` · ${active.place_label}` : ""}</p>
          <h2>{readableLabel(active.title)}</h2>
          {active.summary ? <p>{active.summary}</p> : null}
          <p>{index + 1} / {total}</p>
        </article>
      ) : null}
    </section>
  );
}

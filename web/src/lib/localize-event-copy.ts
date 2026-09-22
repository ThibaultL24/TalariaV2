// web/src/lib/localize-event-copy.ts
import type { GeoJsonFeatureCollection, TimelineEvent } from "@/lib/api";
import { eventTypeLabel } from "@/lib/event-taxonomy";
import type { AppLocale } from "@/lib/i18n";

const PLACE_PAIRS: Array<[string, string]> = [
  ["Moscow", "Moscou"],
  ["Vienna", "Vienne"],
  ["Saint Helena", "Sainte-Hélène"],
  ["St Helena", "Sainte-Hélène"],
  ["Corsica", "Corse"],
  ["Egypt", "Égypte"],
  ["London", "Londres"],
  ["Venice", "Venise"],
  ["Genoa", "Gênes"],
  ["Mantua", "Mantoue"],
  ["Lisbon", "Lisbonne"],
  ["The Hague", "La Haye"],
  ["Brussels", "Bruxelles"],
  ["Antwerp", "Anvers"],
  ["Warsaw", "Varsovie"],
  ["Cairo", "Le Caire"],
  ["Alexandria", "Alexandrie"],
  ["Jerusalem", "Jérusalem"],
  ["Athens", "Athènes"],
  ["Elba", "Île d'Elbe"],
  ["Jena", "Iéna"],
  ["Dresden", "Dresde"],
  ["Spain", "Espagne"],
  ["Italy", "Italie"],
  ["Germany", "Allemagne"],
  ["England", "Angleterre"],
  ["Austria", "Autriche"],
  ["Poland", "Pologne"],
  ["Russia", "Russie"],
  ["Prussia", "Prusse"],
  ["Sicily", "Sicile"],
  ["Netherlands", "Pays-Bas"],
  ["Belgium", "Belgique"],
  ["Switzerland", "Suisse"],
  ["United States", "États-Unis"],
];

const EN_MARKERS = [
  " the ",
  " of ",
  " was ",
  " were ",
  " and ",
  " born ",
  " died ",
  " married ",
  " fought ",
];
const FR_MARKERS = [
  " le ",
  " la ",
  " les ",
  " une ",
  " des ",
  " est ",
  " dans ",
  " naît ",
  " nait ",
  " mort ",
  " bataille ",
];
const EN_KINDS = [
  "birth",
  "death",
  "battle",
  "office",
  "marriage",
  "travel",
  "residence",
  "exile",
  "life event",
  "historical fact",
  "notable event",
];
const FR_KINDS = [
  "naissance",
  "mort",
  "bataille",
  "charge",
  "fonction",
  "mariage",
  "voyage",
  "résidence",
  "exil",
  "fait de vie",
  "fait historique",
  "événement notable",
];

export function localizedPlaceLabel(
  place: string | null | undefined,
  locale: AppLocale,
): string | undefined {
  if (!place?.trim()) return undefined;
  const needle = place.trim().toLowerCase();
  for (const [en, fr] of PLACE_PAIRS) {
    if (needle === en.toLowerCase() || needle === fr.toLowerCase()) {
      return locale === "fr" ? fr : en;
    }
  }
  return place.trim();
}

const PERSON_PAIRS: Array<[string, string]> = [
  ["Napoleon", "Napoléon"],
  ["Napoleon Bonaparte", "Napoléon Bonaparte"],
  ["Joan of Arc", "Jeanne d'Arc"],
  ["Moliere", "Molière"],
];

export function localizedPersonLabel(
  label: string | null | undefined,
  locale: AppLocale,
): string {
  if (!label?.trim()) return "";
  const needle = label.trim().toLowerCase();
  for (const [en, fr] of PERSON_PAIRS) {
    if (needle === en.toLowerCase() || needle === fr.toLowerCase()) {
      return locale === "fr" ? fr : en;
    }
  }
  return label.trim();
}

const MONTH_PAIRS: Array<[string, string]> = [
  ["January", "janvier"],
  ["February", "février"],
  ["March", "mars"],
  ["April", "avril"],
  ["May", "mai"],
  ["June", "juin"],
  ["July", "juillet"],
  ["August", "août"],
  ["September", "septembre"],
  ["October", "octobre"],
  ["November", "novembre"],
  ["December", "décembre"],
];

export function localizedDateSurface(surface: string, locale: AppLocale): string {
  let out = surface;
  for (const [en, fr] of MONTH_PAIRS) {
    const from = locale === "fr" ? en : fr;
    const to = locale === "fr" ? fr : en;
    out = out.replace(new RegExp(`(?<![\\p{L}])${from}(?![\\p{L}])`, "igu"), to);
  }
  return out;
}

function yearFromEvent(event: {
  start_time?: string | null;
  time?: { start?: string | null } | null;
}): number | null {
  const raw = event.time?.start ?? event.start_time;
  if (!raw) return null;
  const year = Number.parseInt(raw.replace(/^-/, "").slice(0, 4), 10);
  return Number.isFinite(year) ? year : null;
}

function titleLooksForeign(title: string, locale: AppLocale): boolean {
  const padded = ` ${title.toLowerCase()} `;
  const enHits = EN_MARKERS.filter((marker) => padded.includes(marker)).length;
  const frHits =
    FR_MARKERS.filter((marker) => padded.includes(marker)).length +
    (/[éèêàùçîï]/i.test(title) ? 1 : 0);
  if (locale === "fr") return enHits > frHits && enHits > 0;
  return frHits > enHits && frHits > 0;
}

function needsStructuredFlip(title: string, locale: AppLocale): boolean {
  const trimmed = title.trim();
  if (/^(born in|died in)\b/i.test(trimmed)) return locale === "fr";
  if (/^(né |née |mort |morte |décédé|décédée)/i.test(trimmed)) return locale === "en";
  if (!trimmed.includes("·")) return false;
  const left = trimmed.split("·")[0]?.trim().toLowerCase() ?? "";
  if (locale === "fr") return EN_KINDS.includes(left);
  return FR_KINDS.includes(left);
}

function bornDiedHeadline(
  locale: AppLocale,
  born: boolean,
  year: number | null,
  place?: string,
): string {
  const fr = locale === "fr";
  if (born && year != null && place) {
    return fr ? `Né à ${place} en ${year}` : `Born in ${place} in ${year}`;
  }
  if (!born && year != null && place) {
    return fr ? `Mort à ${place} en ${year}` : `Died in ${place} in ${year}`;
  }
  if (born && year != null) return fr ? `Né en ${year}` : `Born in ${year}`;
  if (!born && year != null) return fr ? `Mort en ${year}` : `Died in ${year}`;
  if (born && place) return fr ? `Né à ${place}` : `Born in ${place}`;
  if (!born && place) return fr ? `Mort à ${place}` : `Died in ${place}`;
  return eventTypeLabel(born ? "birth" : "death", locale);
}

export function localizedEventTitle(
  event: Pick<TimelineEvent, "title" | "event_type" | "place_label" | "start_time"> & {
    time?: TimelineEvent["time"];
  },
  locale: AppLocale,
): string {
  const title = event.title?.trim() ?? "";
  if (title && !titleLooksForeign(title, locale) && !needsStructuredFlip(title, locale)) {
    return title;
  }
  const year = yearFromEvent(event);
  const place = localizedPlaceLabel(event.place_label, locale);
  if (event.event_type === "birth" || event.event_type === "death") {
    return bornDiedHeadline(locale, event.event_type === "birth", year, place);
  }
  return [eventTypeLabel(event.event_type, locale), year, place]
    .filter((part) => part != null && part !== "")
    .join(" · ");
}

export function localizeFeatureCollection(
  collection: GeoJsonFeatureCollection,
  locale: AppLocale,
): GeoJsonFeatureCollection {
  return {
    type: "FeatureCollection",
    features: collection.features.map((feature) => {
      const props = feature.properties ?? {};
      const eventType = String(props.event_type ?? "");
      const title = localizedEventTitle(
        {
          title: String(props.title ?? ""),
          event_type: eventType,
          place_label: (props.place_label as string | null | undefined) ?? null,
          start_time: (props.start_time as string | null | undefined) ?? null,
          time: props.time as TimelineEvent["time"],
        },
        locale,
      );
      const place = localizedPlaceLabel(
        (props.place_label as string | null | undefined) ?? null,
        locale,
      );
      return {
        ...feature,
        properties: {
          ...props,
          title,
          place_label: place ?? props.place_label,
        },
      };
    }),
  };
}

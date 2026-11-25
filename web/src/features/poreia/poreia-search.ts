// web/src/features/poreia/poreia-search.ts
const KEPT = ["destination", "dlat", "dlon", "radius", "site"] as const;

export function poreiaSearchSuffix(experience: string, params: URLSearchParams): string {
  if (experience !== "poreia") return "";
  const kept = new URLSearchParams();
  for (const key of KEPT) {
    const value = params.get(key);
    if (value) kept.set(key, value);
  }
  const text = kept.toString();
  return text ? `?${text}` : "";
}

export interface PoreiaDestinationQuery {
  qid: string;
  lat: number;
  lon: number;
  radiusKm: number;
}

export function readPoreiaDestination(params: URLSearchParams): PoreiaDestinationQuery | undefined {
  const qid = params.get("destination") ?? "";
  const latText = params.get("dlat");
  const lonText = params.get("dlon");
  if (!latText || !lonText) return undefined;
  const lat = Number(latText);
  const lon = Number(lonText);
  const radiusKm = Number(params.get("radius") ?? "10");
  if (!/^Q[1-9][0-9]*$/.test(qid)) return undefined;
  if (!Number.isFinite(lat) || lat < -90 || lat > 90) return undefined;
  if (!Number.isFinite(lon) || lon < -180 || lon > 180) return undefined;
  if (!Number.isFinite(radiusKm) || radiusKm < 0.1 || radiusKm > 50) return undefined;
  return { qid, lat, lon, radiusKm };
}

export function formatPoreiaDay(isoDate: string, locale: "fr" | "en"): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(isoDate);
  if (!match) return isoDate;
  const date = new Date(Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3])));
  return new Intl.DateTimeFormat(locale === "fr" ? "fr-FR" : "en-GB", {
    day: "numeric",
    month: "short",
    year: "numeric",
    timeZone: "UTC",
  }).format(date);
}

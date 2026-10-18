// web/src/features/poreia/components/poreia-place-card.tsx
import type { HeritagePlace } from "@/features/poreia/heritage-catalog";
import type { VisitSite } from "@/lib/entity-views";
import { useI18n } from "@/lib/i18n";

const RELATIONS: Record<NonNullable<HeritagePlace["relationPid"]>, { fr: string; en: string }> = {
  P547: { fr: "Hommage", en: "Commemorates" },
  P138: { fr: "Nommé en l’honneur de", en: "Named after" },
  P119: { fr: "Sépulture", en: "Burial place" },
  P551: { fr: "Résidence", en: "Residence" },
};

const ACCESS: Record<VisitSite["visitability"], { fr: string; en: string }> = {
  unknown: { fr: "Accès non vérifié", en: "Access not verified" },
  verified_visitable: { fr: "Visite confirmée", en: "Visit verified" },
  verified_closed: { fr: "Fermé au public", en: "Closed to the public" },
  restricted: { fr: "Accès restreint", en: "Restricted access" },
};

export function PoreiaPlaceCard({ place, heading = true }: { place: HeritagePlace; heading?: boolean }) {
  const { locale } = useI18n();
  const relation = place.relationPid ? RELATIONS[place.relationPid][locale] : null;

  return (
    <article className="poreia-place-card">
      <figure className="poreia-place-card__figure">
        <img src="/brand/poreia-scene.webp" alt="" />
        <figcaption>
          {locale === "fr"
            ? "Illustration générique. Ce n’est pas une photographie du lieu."
            : "Generic illustration. This is not a photograph of the place."}
        </figcaption>
      </figure>
      {heading ? <h3>{place.title}</h3> : null}
      <p>{relation ? `${relation} · ${place.kind.replaceAll("_", " ")}` : place.kind.replaceAll("_", " ")}</p>
      {place.city ? <p>{place.city}</p> : null}
      <p>{ACCESS[place.visitability][locale]}</p>
      {place.openingHours && place.hoursCheckedAt ? (
        <p>
          {locale === "fr"
            ? `Horaires renseignés par OSM le ${place.hoursCheckedAt.slice(0, 10)} : ${place.openingHours}`
            : `Hours recorded by OSM on ${place.hoursCheckedAt.slice(0, 10)}: ${place.openingHours}`}
        </p>
      ) : null}
      {place.summary ? <p>{place.summary}</p> : null}
      {place.sources.map((source) => source.url ? (
        <a key={source.url} href={source.url} target="_blank" rel="noopener noreferrer">
          {locale === "fr" ? `Source de l’association · ${source.label}` : `Association source · ${source.label}`}
        </a>
      ) : null)}
    </article>
  );
}

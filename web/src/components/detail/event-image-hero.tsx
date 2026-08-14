// web/src/components/detail/event-image-hero.tsx
import type { ResolvedEventImage } from "@/lib/resolve-event-image";

interface EventImageHeroProps {
  image: ResolvedEventImage | null;
  loading: boolean;
}

export function EventImageHero({ image, loading }: EventImageHeroProps) {
  if (loading) {
    return (
      <div
        className="h-48 w-full animate-pulse rounded-xl bg-(--color-bg-primary)/60"
        aria-hidden
      />
    );
  }
  if (!image) return null;

  return (
    <figure className="overflow-hidden rounded-xl border border-(--color-border-subtle)">
      <a
        href={image.pageUrl}
        target="_blank"
        rel="noopener noreferrer"
        className="flex h-48 w-full items-center justify-center bg-(--color-bg-primary)/50"
      >
        <img
          src={image.url}
          alt={image.pageTitle}
          className="max-h-48 max-w-full object-contain object-center"
          loading="lazy"
        />
      </a>
      <figcaption className="border-t border-(--color-border-subtle) bg-(--color-bg-primary)/35 px-3 py-2 text-[11px] leading-snug text-(--color-text-muted)">
        <span className="font-medium text-(--color-text-secondary)">{image.pageTitle}</span>
        {" · "}
        <a
          href={image.pageUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="text-(--color-accent-strong) hover:underline"
        >
          Wikipedia / Wikimedia Commons
        </a>
      </figcaption>
    </figure>
  );
}

// web/src/components/detail/source-refs-list.tsx
import type { EventSourceRef } from "@/lib/api";
import { resolveSourceParagraphHref } from "@/components/detail/source-ref-url";
import { useI18n } from "@/lib/i18n";

interface SourceRefsListProps {
  refs: EventSourceRef[];
  wikiLang?: string;
  activeCitationIndex?: number | null;
}

export function SourceRefsList({
  refs,
  wikiLang,
  activeCitationIndex = null,
}: SourceRefsListProps) {
  const { t } = useI18n();
  if (refs.length === 0) {
    return <p className="text-sm text-(--color-text-muted)">{t.noSources}</p>;
  }

  return (
    <div className="space-y-2">
      {refs.map((ref, index) => {
        const href = resolveSourceParagraphHref(ref, wikiLang);
        const citeIndex = ref.citation_index ?? index + 1;
        const label =
          ref.page_title?.trim() ||
          ref.source_page_title?.trim() ||
          ref.label?.trim() ||
          `${t.sources} ${citeIndex}`;
        const snippet = ref.snippet ?? ref.quote;
        const isActive = activeCitationIndex === citeIndex;

        return (
          <article
            id={`source-ref-${citeIndex}`}
            key={`${ref.evidence_id ?? label}-${citeIndex}`}
            className={`rounded-lg border px-3 py-2 ${
              isActive
                ? "border-(--color-accent) bg-(--color-accent)/10"
                : "border-(--color-border-subtle) bg-(--color-bg-primary)/40"
            }`}
          >
            <div className="flex items-start justify-between gap-2">
              <p className="text-sm font-medium text-(--color-text-primary)">
                [{citeIndex}] {label}
                {ref.section_title ? (
                  <span className="font-normal text-(--color-text-secondary)">
                    {" "}
                    — {ref.section_title}
                  </span>
                ) : null}
              </p>
              {href ? (
                <a
                  href={href}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="shrink-0 text-[11px] font-medium text-(--color-accent-strong) hover:underline"
                >
                  {t.openParagraph}
                </a>
              ) : null}
            </div>
            {snippet ? (
              <blockquote className="mt-1.5 line-clamp-3 text-[12px] leading-relaxed text-(--color-text-secondary)">
                “{snippet}”
              </blockquote>
            ) : null}
          </article>
        );
      })}
    </div>
  );
}

// web/src/components/detail/source-refs-list.tsx
import type { EventSourceRef } from "@/lib/api";
import { resolveSourceRefHref } from "@/components/detail/source-ref-url";

interface SourceRefsListProps {
  refs: EventSourceRef[];
  wikiLang?: string;
  activeCitationIndex?: number | null;
}

function InlineCitation({ text }: { text: string }) {
  const trimmed = text.trim();
  if (/^https?:\/\//i.test(trimmed)) {
    return (
      <a
        href={trimmed}
        target="_blank"
        rel="noopener noreferrer"
        className="break-all text-(--color-accent-strong) hover:underline"
      >
        {trimmed}
      </a>
    );
  }
  return <span>{text}</span>;
}

export function SourceRefsList({
  refs,
  wikiLang,
  activeCitationIndex = null,
}: SourceRefsListProps) {
  if (refs.length === 0) {
    return (
      <p className="text-sm text-(--color-text-muted)">
        No linked sources were returned by the server for this event.
      </p>
    );
  }

  return (
    <div className="space-y-4">
      {refs.map((ref, index) => {
        const href = resolveSourceRefHref(ref, wikiLang);
        const citeIndex = ref.citation_index ?? index + 1;
        const label =
          ref.page_title?.trim() ||
          ref.source_page_title?.trim() ||
          ref.label?.trim() ||
          `Source ${citeIndex}`;
        const snippet = ref.snippet ?? ref.quote;
        const oldid = ref.oldid ?? ref.revision_id;
        const sourceKind =
          ref.source_system === "wikidata"
            ? "Wikidata"
            : ref.page_title || ref.source_page_title
              ? "Wikipedia"
              : "Source";
        const isActive = activeCitationIndex === citeIndex;

        return (
          <article
            id={`source-ref-${citeIndex}`}
            key={`${ref.evidence_id ?? label}-${citeIndex}`}
            className={`rounded-xl border p-4 ${
              isActive
                ? "border-(--color-accent) bg-(--color-accent)/10"
                : "border-(--color-border-subtle) bg-(--color-bg-primary)/40"
            }`}
          >
            <div className="mb-2 flex flex-wrap items-center justify-between gap-2">
              <span className="text-[10px] font-semibold uppercase tracking-wide text-(--color-text-secondary)">
                [{citeIndex}] {sourceKind}
              </span>
              {href ? (
                <a
                  href={href}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="shrink-0 rounded-lg bg-(--color-accent) px-3 py-1.5 text-xs font-medium text-(--color-bg-elevated) shadow-sm hover:opacity-95"
                >
                  Open source
                </a>
              ) : null}
            </div>

            <h4 className="text-sm font-semibold leading-snug text-(--color-text-primary)">
              {href ? (
                <a
                  href={href}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-(--color-accent-strong) hover:underline"
                >
                  {label}
                </a>
              ) : (
                label
              )}
              {ref.section_title ? (
                <span className="font-normal text-(--color-text-secondary)">
                  {" "}
                  — {ref.section_title}
                </span>
              ) : null}
            </h4>

            {href ? (
              <p
                className="mt-2 break-all font-mono text-[11px] leading-relaxed text-(--color-text-secondary)"
                title={href}
              >
                {href}
              </p>
            ) : (
              <p className="mt-2 rounded-md border border-(--color-border-subtle) bg-(--color-bg-primary) px-2 py-1.5 text-xs leading-snug text-(--color-text-primary)">
                No direct link — missing title or identifiers in metadata.
              </p>
            )}

            {oldid != null ? (
              <div className="mt-2 text-[11px] text-(--color-text-muted)">
                Wikipedia revision (oldid) · {oldid}
              </div>
            ) : (
              <div className="mt-2 text-[11px] text-(--color-text-muted)">
                Revision not pinned — article URL only
              </div>
            )}

            {ref.sentence_ordinal != null || ref.offset_start != null ? (
              <div className="mt-1 text-[11px] text-(--color-text-muted)">
                {ref.sentence_ordinal != null ? `Sentence #${ref.sentence_ordinal}` : null}
                {ref.sentence_ordinal != null && ref.offset_start != null ? " · " : null}
                {ref.offset_start != null && ref.offset_end != null
                  ? `chars ${ref.offset_start}–${ref.offset_end}`
                  : null}
              </div>
            ) : null}

            {snippet ? (
              <blockquote className="mt-3 border-l-2 border-(--color-accent)/40 pl-3 text-sm leading-relaxed text-(--color-text-primary)">
                “{snippet}”
              </blockquote>
            ) : null}

            {ref.inline_citations?.length ? (
              <div className="mt-3 space-y-1 text-xs">
                <span className="font-medium text-(--color-text-secondary)">References:</span>
                <ul className="list-inside list-disc space-y-1 pl-1 text-(--color-text-primary)">
                  {ref.inline_citations.map((citation, citationIndex) => (
                    <li key={`${citationIndex}-${citation.slice(0, 24)}`}>
                      <InlineCitation text={citation} />
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}
          </article>
        );
      })}
    </div>
  );
}

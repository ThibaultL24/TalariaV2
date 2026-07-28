// web/src/components/detail/cited-paragraph.tsx
interface CitedParagraphProps {
  text: string;
  onCiteClick?: (index: number) => void;
}

/** Renders dossier prose and turns [n] markers into clickable citation chips. */
export function CitedParagraph({ text, onCiteClick }: CitedParagraphProps) {
  const parts = text.split(/(\[\d+\])/g);

  return (
    <p className="whitespace-pre-wrap text-sm leading-relaxed text-(--color-text-primary)">
      {parts.map((part, index) => {
        const match = part.match(/^\[(\d+)\]$/);
        if (!match) return <span key={`${index}-${part.slice(0, 12)}`}>{part}</span>;
        const cite = Number(match[1]);
        return (
          <button
            key={`cite-${cite}-${index}`}
            type="button"
            onClick={() => onCiteClick?.(cite)}
            className="mx-0.5 inline-flex translate-y-[-0.15em] items-center rounded bg-(--color-accent)/15 px-1 py-0.5 text-[10px] font-semibold text-(--color-accent-strong) hover:bg-(--color-accent)/25"
            aria-label={`Open citation ${cite}`}
          >
            [{cite}]
          </button>
        );
      })}
    </p>
  );
}

// web/src/components/detail/cited-paragraph.tsx
interface CitedParagraphProps {
  text: string;
  onCiteClick?: (index: number) => void;
  variant?: "lead" | "body";
}

/** Renders dossier prose and turns [n] markers into editorial citation marks. */
export function CitedParagraph({
  text,
  onCiteClick,
  variant = "body",
}: CitedParagraphProps) {
  const parts = text.split(/(\[\d+\])/g);
  const isLead = variant === "lead";

  return (
    <p
      className={
        isLead
          ? "text-[15px] leading-[1.55] text-(--color-text-primary)"
          : "text-[13.5px] leading-[1.7] text-(--color-text-secondary)"
      }
      style={isLead ? { fontFamily: "var(--font-display)" } : undefined}
    >
      {parts.map((part, index) => {
        const match = part.match(/^\[(\d+)\]$/);
        if (!match) {
          return <span key={`${index}-${part.slice(0, 12)}`}>{part}</span>;
        }
        const cite = Number(match[1]);
        return (
          <button
            key={`cite-${cite}-${index}`}
            type="button"
            onClick={() => onCiteClick?.(cite)}
            className="dossier-cite ml-0.5 inline-flex h-[1.15em] min-w-[1.15em] translate-y-[-0.35em] items-center justify-center rounded-full bg-(--color-accent-soft) px-1 align-super text-[10px] font-semibold leading-none text-(--color-accent-strong) transition hover:bg-(--color-accent)/25"
            aria-label={`Open citation ${cite}`}
          >
            {cite}
          </button>
        );
      })}
    </p>
  );
}

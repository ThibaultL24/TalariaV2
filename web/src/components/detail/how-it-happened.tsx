// web/src/components/detail/how-it-happened.tsx
import { CitedParagraph } from "@/components/detail/cited-paragraph";
import { useI18n } from "@/lib/i18n";

interface HowItHappenedProps {
  text: string;
  onCiteClick?: (index: number) => void;
}

interface ParsedDossier {
  lead: string | null;
  body: string[];
}

/** Strip machine STATEMENT lines and split framing lead from source prose. */
export function parseDossierProse(raw: string): ParsedDossier {
  const cleaned = raw
    .replace(/\bSTATEMENT\b[^\[]*\[\d+\]/gi, " ")
    .replace(/\bSTATEMENT\b[^.!?\[]*/gi, " ")
    .replace(/\s+/g, " ")
    .trim();

  if (!cleaned) return { lead: null, body: [] };

  const sentences =
    cleaned.match(/[^.!?]+[.!?]+(?:\s*\[\d+\])?|[^.!?]+$/g)?.map((s) => s.trim()).filter(Boolean) ??
    [cleaned];

  const lead = sentences[0] ?? null;
  const rest = sentences.slice(1);
  const body: string[] = [];
  for (let i = 0; i < rest.length; i += 2) {
    body.push(rest.slice(i, i + 2).join(" "));
  }
  return { lead, body };
}

export function HowItHappened({ text, onCiteClick }: HowItHappenedProps) {
  const { t } = useI18n();
  const { lead, body } = parseDossierProse(text);
  if (!lead && body.length === 0) return null;

  return (
    <section className="dossier-how overflow-hidden rounded-xl border border-(--color-border-subtle) bg-(--color-bg-primary)/40">
      <header className="border-b border-(--color-border-subtle) px-4 py-3">
        <p className="text-[10px] font-semibold uppercase tracking-[0.14em] text-(--color-accent-strong)">
          {t.summary}
        </p>
        <h4
          className="mt-0.5 text-sm font-semibold tracking-wide text-(--color-text-primary)"
          style={{ fontFamily: "var(--font-display)" }}
        >
          {t.dossierTitle}
        </h4>
      </header>

      <div className="space-y-4 px-4 py-4">
        {lead ? (
          <CitedParagraph text={lead} onCiteClick={onCiteClick} variant="lead" />
        ) : null}

        {body.length > 0 ? (
          <div className="space-y-3 border-t border-(--color-border-subtle)/70 pt-4">
            {body.map((paragraph, index) => (
              <CitedParagraph
                key={`dossier-p-${index}`}
                text={paragraph}
                onCiteClick={onCiteClick}
                variant="body"
              />
            ))}
          </div>
        ) : null}
      </div>

      <footer className="border-t border-(--color-border-subtle) bg-(--color-bg-elevated)/50 px-4 py-2.5">
        <p className="text-[11px] leading-relaxed text-(--color-text-muted)">
          {t.dossierHint}
        </p>
      </footer>
    </section>
  );
}

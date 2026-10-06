// web/src/components/agora/argument-form.tsx
import { useState } from "react";
import type { ArgumentRelation, BibliographyItem } from "@/lib/api";
import { useI18n } from "@/lib/i18n";

interface ArgumentFormProps {
  bibliography: BibliographyItem[];
  submitLabel: string;
  initialStatement?: string;
  onSubmit: (payload: {
    relation: ArgumentRelation;
    statement: string;
    corpus_document_id: string;
    quote: string;
  }) => Promise<void>;
}

export function ArgumentForm({
  bibliography,
  submitLabel,
  initialStatement = "",
  onSubmit,
}: ArgumentFormProps) {
  const { t } = useI18n();
  const [relation, setRelation] = useState<ArgumentRelation>("supports");
  const [statement, setStatement] = useState(initialStatement);
  const [quote, setQuote] = useState("");
  const [documentId, setDocumentId] = useState(bibliography[0]?.id ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (!documentId) {
      setError(t.sourceNotIndexed);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await onSubmit({
        relation,
        statement,
        corpus_document_id: documentId,
        quote,
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : t.sectionError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <form className="argument-form" onSubmit={(event) => void submit(event)}>
      <fieldset>
        <legend>{t.argumentRelation}</legend>
        {(["supports", "contradicts", "qualifies"] as const).map((value) => (
          <label key={value}>
            <input
              type="radio"
              name="argument-relation"
              checked={relation === value}
              onChange={() => setRelation(value)}
            />
            {value === "supports"
              ? t.argumentSupports
              : value === "contradicts"
                ? t.argumentContradicts
                : t.argumentQualifies}
          </label>
        ))}
      </fieldset>
      <label htmlFor="argument-statement">
        {t.argumentStatement}
        <textarea
          id="argument-statement"
          value={statement}
          onChange={(event) => setStatement(event.target.value)}
          required
        />
      </label>
      <label htmlFor="argument-source">
        {t.selectSource}
        <select
          id="argument-source"
          value={documentId}
          onChange={(event) => setDocumentId(event.target.value)}
          required
        >
          {bibliography.map((item) => (
            <option key={item.id} value={item.id}>
              {item.title}
            </option>
          ))}
        </select>
      </label>
      <label htmlFor="argument-evidence">
        {t.argumentEvidence}
        <textarea
          id="argument-evidence"
          value={quote}
          onChange={(event) => setQuote(event.target.value)}
          required
        />
      </label>
      {error ? <p role="alert">{error}</p> : null}
      <button type="submit" disabled={busy || bibliography.length === 0}>
        {submitLabel}
      </button>
    </form>
  );
}

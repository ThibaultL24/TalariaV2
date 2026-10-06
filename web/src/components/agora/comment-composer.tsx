// web/src/components/agora/comment-composer.tsx
import { useState } from "react";
import { useI18n } from "@/lib/i18n";

interface CommentComposerProps {
  placeholder: string;
  submitLabel: string;
  disabled?: boolean;
  onSubmit: (body: string) => Promise<void>;
}

export function CommentComposer({
  placeholder,
  submitLabel,
  disabled,
  onSubmit,
}: CommentComposerProps) {
  const { t } = useI18n();
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit() {
    const trimmed = body.trim();
    if (!trimmed || disabled) return;
    setBusy(true);
    setError(null);
    try {
      await onSubmit(trimmed);
      setBody("");
    } catch (err) {
      setError(err instanceof Error ? err.message : t.signalFailed);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="comment-composer">
      <textarea
        className="comment-composer__input"
        value={body}
        maxLength={4000}
        disabled={disabled || busy}
        placeholder={placeholder}
        onChange={(e) => setBody(e.target.value)}
      />
      <button
        type="button"
        className="intuition-stance-btn"
        disabled={disabled || busy || !body.trim()}
        onClick={() => void submit()}
      >
        {submitLabel}
      </button>
      {error ? (
        <p className="mt-1 text-[11px] text-(--color-trust-low)" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}

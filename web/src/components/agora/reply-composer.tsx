// web/src/components/agora/reply-composer.tsx
import { CommentComposer } from "@/components/agora/comment-composer";
import { useI18n } from "@/lib/i18n";

interface ReplyComposerProps {
  disabled?: boolean;
  onSubmit: (body: string) => Promise<void>;
}

export function ReplyComposer({ disabled, onSubmit }: ReplyComposerProps) {
  const { t } = useI18n();
  return (
    <CommentComposer
      placeholder={t.replyPlaceholder}
      submitLabel={t.postReply}
      disabled={disabled}
      onSubmit={onSubmit}
    />
  );
}

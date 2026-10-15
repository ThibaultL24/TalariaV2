// web/src/components/agora/comment-card.tsx
import { useState } from "react";
import { AgoraModal } from "@/components/agora/agora-modal";
import { ArgumentForm } from "@/components/agora/argument-form";
import { CommentReactionBar } from "@/components/agora/comment-reaction-bar";
import { ReplyComposer } from "@/components/agora/reply-composer";
import { promoteCommentToArgument, type AgoraComment, type BibliographyItem } from "@/lib/api";
import { useI18n } from "@/lib/i18n";

interface CommentCardProps {
  comment: AgoraComment;
  canWrite: boolean;
  currentUserId?: string | null;
  bibliography?: BibliographyItem[];
  onSignIn: () => void;
  onChange: (next: AgoraComment) => void;
  onReply?: (body: string) => Promise<void>;
  onPromoted?: () => void;
}

export function CommentCard({
  comment,
  canWrite,
  currentUserId,
  bibliography = [],
  onSignIn,
  onChange,
  onReply,
  onPromoted,
}: CommentCardProps) {
  const { t } = useI18n();
  const [replyOpen, setReplyOpen] = useState(false);
  const [promoteOpen, setPromoteOpen] = useState(false);
  const author = comment.author.display_name?.trim() || t.anonymousHistorian;
  const deleted = comment.status === "deleted";
  const isOwn = Boolean(currentUserId && currentUserId === comment.author.id);

  return (
    <article className="comment-card">
      <p className="comment-card__meta">
        {author}
        {comment.edited_at && !deleted ? ` · ${t.commentEdited}` : null}
      </p>
      {deleted ? (
        <p className="comment-card__deleted">{t.commentDeleted}</p>
      ) : (
        <p className="comment-card__body">{comment.body}</p>
      )}
      <CommentReactionBar
        comment={comment}
        canReact={canWrite}
        onSignIn={onSignIn}
        onChange={onChange}
      />
      {onReply ? (
        <button
          type="button"
          className="comment-card__reply-toggle"
          onClick={() => {
            if (!canWrite) {
              onSignIn();
              return;
            }
            setReplyOpen((open) => !open);
          }}
        >
          {t.replyPlaceholder}
        </button>
      ) : null}
      {isOwn && !deleted ? (
        <button
          type="button"
          className="comment-card__reply-toggle"
          onClick={() => setPromoteOpen(true)}
        >
          {t.promoteToArgument}
        </button>
      ) : null}
      {onReply && replyOpen ? (
        <ReplyComposer
          disabled={!canWrite}
          onSubmit={async (body) => {
            await onReply(body);
            setReplyOpen(false);
          }}
        />
      ) : null}
      {comment.replies && comment.replies.length > 0 ? (
        <div className="comment-card__replies">
          {comment.replies.map((reply) => (
            <CommentCard
              key={reply.id}
              comment={reply}
              canWrite={canWrite}
              currentUserId={currentUserId}
              bibliography={bibliography}
              onSignIn={onSignIn}
              onChange={(next) =>
                onChange({
                  ...comment,
                  replies: (comment.replies ?? []).map((item) =>
                    item.id === next.id ? next : item,
                  ),
                })
              }
              onPromoted={onPromoted}
            />
          ))}
        </div>
      ) : null}
      <AgoraModal
        title={t.promoteToArgument}
        open={promoteOpen}
        onClose={() => setPromoteOpen(false)}
      >
        {bibliography.length ? (
          <ArgumentForm
            bibliography={bibliography}
            submitLabel={t.submitArgument}
            initialStatement={comment.body ?? ""}
            onSubmit={async (payload) => {
              await promoteCommentToArgument(comment.id, payload);
              setPromoteOpen(false);
              onPromoted?.();
            }}
          />
        ) : (
          <p>{t.sourceNotIndexed}</p>
        )}
      </AgoraModal>
    </article>
  );
}

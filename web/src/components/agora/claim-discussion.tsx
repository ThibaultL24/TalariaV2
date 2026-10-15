// web/src/components/agora/claim-discussion.tsx
import { useEffect, useState } from "react";
import { CommentComposer } from "@/components/agora/comment-composer";
import { CommentList } from "@/components/agora/comment-list";
import {
  fetchClaimComments,
  postClaimComment,
  type AgoraComment,
  type BibliographyItem,
} from "@/lib/api";
import { useI18n } from "@/lib/i18n";
import { useTalariaSession } from "@/hooks/use-talaria-session";

interface ClaimDiscussionProps {
  claimId: string;
  commentCount: number;
  bibliography?: BibliographyItem[];
  forceOpen?: boolean;
  onCountChanged?: () => void;
}

export function ClaimDiscussion({
  claimId,
  commentCount,
  bibliography,
  forceOpen = false,
  onCountChanged,
}: ClaimDiscussionProps) {
  const { t } = useI18n();
  const session = useTalariaSession();
  const [open, setOpen] = useState(forceOpen);
  const [items, setItems] = useState<AgoraComment[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  async function load() {
    setLoading(true);
    setError(null);
    try {
      const data = await fetchClaimComments(claimId);
      setItems(data.items);
      setLoaded(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : t.sectionError);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    if (forceOpen) void load();
  }, [forceOpen, claimId]);

  async function toggle() {
    const next = !open;
    setOpen(next);
    if (next && !loaded) await load();
  }

  function requireSession(): boolean {
    if (session.authenticated) return true;
    setMessage(t.commentSignInRequired);
    return false;
  }

  async function onCreate(body: string) {
    if (!requireSession()) throw new Error(t.commentSignInRequired);
    const { comment } = await postClaimComment(claimId, body);
    setItems((prev) => [{ ...comment, replies: comment.replies ?? [] }, ...prev]);
    onCountChanged?.();
  }

  async function onReply(parentId: string, body: string) {
    if (!requireSession()) throw new Error(t.commentSignInRequired);
    const { comment } = await postClaimComment(claimId, body, parentId);
    setItems((prev) =>
      prev.map((root) =>
        root.id === parentId
          ? {
              ...root,
              replies: [...(root.replies ?? []), comment],
              reply_count: (root.reply_count ?? 0) + 1,
            }
          : root,
      ),
    );
    onCountChanged?.();
  }

  return (
    <div className="claim-discussion">
      {forceOpen ? (
        <h3 className="claim-section-heading">{t.commentsTitle}</h3>
      ) : (
        <button type="button" className="claim-discussion__toggle" onClick={() => void toggle()}>
          {open ? t.closeDiscussion : t.openDiscussion} · {t.comments(commentCount)}
        </button>
      )}
      {open ? (
        <div className="claim-discussion__panel">
          {loading ? <p role="status">{t.loading}</p> : null}
          {error ? (
            <p role="alert">
              {error}{" "}
              <button type="button" onClick={() => void load()}>
                {t.retrySection}
              </button>
            </p>
          ) : null}
          <CommentComposer
            placeholder={t.commentPlaceholder}
            submitLabel={t.postComment}
            onSubmit={onCreate}
          />
          {!loading && loaded && items.length === 0 ? (
            <p className="claim-section-empty">{t.noCommentsYet}</p>
          ) : null}
          <CommentList
            comments={items}
            canWrite={session.authenticated}
            currentUserId={session.user?.id}
            bibliography={bibliography}
            onSignIn={() => setMessage(t.commentSignInRequired)}
            onChange={(next) =>
              setItems((prev) => prev.map((item) => (item.id === next.id ? next : item)))
            }
            onReply={onReply}
            onPromoted={onCountChanged}
          />
        </div>
      ) : null}
      {message ? (
        <p className="mt-1.5 text-[11px] text-(--color-trust-low)" role="alert">
          {message}
        </p>
      ) : null}
    </div>
  );
}

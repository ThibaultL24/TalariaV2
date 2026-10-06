// web/src/components/agora/comment-list.tsx
import { CommentCard } from "@/components/agora/comment-card";
import { type AgoraComment, type BibliographyItem } from "@/lib/api";

interface CommentListProps {
  comments: AgoraComment[];
  canWrite: boolean;
  currentUserId?: string | null;
  bibliography?: BibliographyItem[];
  onSignIn: () => void;
  onChange: (next: AgoraComment) => void;
  onReply: (parentId: string, body: string) => Promise<void>;
  onPromoted?: () => void;
}

export function CommentList({
  comments,
  canWrite,
  currentUserId,
  bibliography,
  onSignIn,
  onChange,
  onReply,
  onPromoted,
}: CommentListProps) {
  return (
    <div className="comment-list">
      {comments.map((comment) => (
        <CommentCard
          key={comment.id}
          comment={comment}
          canWrite={canWrite}
          currentUserId={currentUserId}
          bibliography={bibliography}
          onSignIn={onSignIn}
          onChange={onChange}
          onReply={(body) => onReply(comment.id, body)}
          onPromoted={onPromoted}
        />
      ))}
    </div>
  );
}

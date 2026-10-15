// web/src/components/agora/comment-reaction-bar.tsx
import {
  addCommentReaction,
  removeCommentReaction,
  type AgoraComment,
  type CommentReactionType,
} from "@/lib/api";
import { useI18n } from "@/lib/i18n";

const TYPES: CommentReactionType[] = [
  "relevant",
  "well_sourced",
  "interesting",
  "needs_nuance",
  "disagree",
];

interface CommentReactionBarProps {
  comment: AgoraComment;
  canReact: boolean;
  onSignIn: () => void;
  onChange: (next: AgoraComment) => void;
}

function toggleLocal(
  comment: AgoraComment,
  type: CommentReactionType,
  adding: boolean,
): AgoraComment {
  const mine = adding
    ? [...comment.my_reactions, type]
    : comment.my_reactions.filter((item) => item !== type);
  const n = (comment.reactions[type] ?? 0) + (adding ? 1 : -1);
  return {
    ...comment,
    my_reactions: mine,
    reactions: { ...comment.reactions, [type]: Math.max(0, n) },
  };
}

export function CommentReactionBar({
  comment,
  canReact,
  onSignIn,
  onChange,
}: CommentReactionBarProps) {
  const { t } = useI18n();
  const labels: Record<CommentReactionType, string> = {
    relevant: t.reactionRelevant,
    well_sourced: t.reactionWellSourced,
    interesting: t.reactionInteresting,
    needs_nuance: t.reactionNeedsNuance,
    disagree: t.reactionDisagree,
  };

  async function onToggle(type: CommentReactionType) {
    if (!canReact) {
      onSignIn();
      return;
    }
    const has = comment.my_reactions.includes(type);
    const previous = comment;
    onChange(toggleLocal(comment, type, !has));
    try {
      const result = has
        ? await removeCommentReaction(comment.id, type)
        : await addCommentReaction(comment.id, type);
      onChange(result.comment);
    } catch {
      onChange(previous);
    }
  }

  if (comment.status === "deleted") return null;

  return (
    <div className="comment-reaction-bar">
      {TYPES.map((type) => {
        const mine = comment.my_reactions.includes(type);
        return (
          <button
            key={type}
            type="button"
            className={`comment-reaction${mine ? " is-mine" : ""}`}
            onClick={() => void onToggle(type)}
          >
            {labels[type]} {comment.reactions[type] ?? 0}
          </button>
        );
      })}
    </div>
  );
}

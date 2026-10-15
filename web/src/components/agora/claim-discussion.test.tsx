// web/src/components/agora/claim-discussion.test.tsx
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ClaimDiscussion } from "./claim-discussion";
import { ClaimActionBar } from "./claim-action-bar";

const sessionState = vi.hoisted(() => ({
  authenticated: false,
  user: null as { id: string } | null,
}));

const fetchClaimComments = vi.fn();
const postClaimComment = vi.fn();
const addCommentReaction = vi.fn();
const removeCommentReaction = vi.fn();
const promoteCommentToArgument = vi.fn();

vi.mock("@/hooks/use-talaria-session", () => ({
  useTalariaSession: () => sessionState,
}));

vi.mock("@/hooks/use-intuition-wallet", () => ({
  useIntuitionWallet: () => ({
    address: undefined,
    chainId: 13579,
    isConnected: false,
    isSupportedChain: true,
  }),
}));

vi.mock("wagmi", () => ({
  useSwitchChain: () => ({ switchChain: vi.fn() }),
}));

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return {
    ...actual,
    fetchClaimComments: (...args: unknown[]) => fetchClaimComments(...args),
    postClaimComment: (...args: unknown[]) => postClaimComment(...args),
    addCommentReaction: (...args: unknown[]) => addCommentReaction(...args),
    removeCommentReaction: (...args: unknown[]) => removeCommentReaction(...args),
    promoteCommentToArgument: (...args: unknown[]) => promoteCommentToArgument(...args),
  };
});

const rootComment = {
  id: "c1",
  claim_id: "claim-1",
  author: { id: "u1", display_name: null },
  body: "Fragile thesis",
  status: "active" as const,
  edited_at: null,
  created_at: "2026-01-01T00:00:00Z",
  reactions: {
    relevant: 1,
    well_sourced: 0,
    interesting: 0,
    needs_nuance: 0,
    disagree: 0,
  },
  my_reactions: [] as string[],
  replies: [] as unknown[],
  reply_count: 0,
};

describe("ClaimDiscussion", () => {
  beforeEach(() => {
    sessionState.authenticated = false;
    sessionState.user = null;
    fetchClaimComments.mockReset();
    postClaimComment.mockReset();
    addCommentReaction.mockReset();
    removeCommentReaction.mockReset();
    promoteCommentToArgument.mockReset();
    fetchClaimComments.mockResolvedValue({ items: [rootComment], next_cursor: null });
  });

  afterEach(() => {
    cleanup();
  });

  it("shows comment count from the summary without fetching", () => {
    render(
      <>
        <ClaimActionBar
          claimId="claim-1"
          claimText="Thesis"
          summary={{
            target_id: "claim-1",
            counts: { support: 42, dispute: 18, uncertain: 11, save: 0 },
            comment_count: 27,
            mine: [],
          }}
        />
        <ClaimDiscussion claimId="claim-1" commentCount={27} />
      </>,
    );
    expect(screen.getAllByText(/27 comments/i).length).toBeGreaterThanOrEqual(1);
    expect(fetchClaimComments).not.toHaveBeenCalled();
  });

  it("fetches comments only when the discussion is opened", async () => {
    render(<ClaimDiscussion claimId="claim-1" commentCount={1} />);
    expect(fetchClaimComments).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: /open comments/i }));
    await waitFor(() => expect(fetchClaimComments).toHaveBeenCalledTimes(1));
    expect(screen.getByText("Fragile thesis")).toBeTruthy();
  });

  it("prompts sign-in when anonymous users try to comment", async () => {
    render(<ClaimDiscussion claimId="claim-1" commentCount={0} />);
    fireEvent.click(screen.getByRole("button", { name: /open comments/i }));
    await waitFor(() => expect(fetchClaimComments).toHaveBeenCalled());
    fireEvent.change(screen.getByPlaceholderText(/why do you support/i), {
      target: { value: "Hello" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Post comment" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toMatch(/sign in/i));
    expect(postClaimComment).not.toHaveBeenCalled();
  });

  it("appends a created comment and a reply", async () => {
    sessionState.authenticated = true;
    sessionState.user = { id: "u2" };
    postClaimComment
      .mockResolvedValueOnce({
        comment: { ...rootComment, id: "c2", body: "New comment", replies: [] },
      })
      .mockResolvedValueOnce({
        comment: { ...rootComment, id: "r1", body: "A reply", replies: undefined },
      });
    fetchClaimComments.mockResolvedValue({ items: [], next_cursor: null });
    render(<ClaimDiscussion claimId="claim-1" commentCount={0} />);
    fireEvent.click(screen.getByRole("button", { name: /open comments/i }));
    await waitFor(() => expect(fetchClaimComments).toHaveBeenCalled());
    fireEvent.change(screen.getByPlaceholderText(/why do you support/i), {
      target: { value: "New comment" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Post comment" }));
    await waitFor(() => expect(screen.getByText("New comment")).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: "Reply" }));
    fireEvent.change(screen.getByPlaceholderText("Reply"), {
      target: { value: "A reply" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Post reply" }));
    await waitFor(() => expect(screen.getByText("A reply")).toBeTruthy());
    expect(postClaimComment.mock.calls[1][2]).toBe("c2");
  });

  it("renders a placeholder for deleted comments", async () => {
    fetchClaimComments.mockResolvedValue({
      items: [{ ...rootComment, status: "deleted", body: null }],
      next_cursor: null,
    });
    render(<ClaimDiscussion claimId="claim-1" commentCount={1} />);
    fireEvent.click(screen.getByRole("button", { name: /open comments/i }));
    await waitFor(() => expect(screen.getByText("Comment deleted")).toBeTruthy());
  });

  it("updates reaction counts optimistically and rolls back on failure", async () => {
    sessionState.authenticated = true;
    sessionState.user = { id: "u2" };
    addCommentReaction.mockRejectedValue(new Error("nope"));
    render(<ClaimDiscussion claimId="claim-1" commentCount={1} />);
    fireEvent.click(screen.getByRole("button", { name: /open comments/i }));
    await waitFor(() => expect(screen.getByText("Fragile thesis")).toBeTruthy());
    const button = screen.getByRole("button", { name: /relevant 1/i });
    fireEvent.click(button);
    await waitFor(() => expect(screen.getByRole("button", { name: /relevant 2/i })).toBeTruthy());
    await waitFor(() => expect(screen.getByRole("button", { name: /relevant 1/i })).toBeTruthy());
  });

  it("promotes an owned comment into a structured argument without deleting it", async () => {
    sessionState.authenticated = true;
    sessionState.user = { id: "u1" };
    promoteCommentToArgument.mockResolvedValue({
      argument: {
        id: "a1",
        target_claim_id: "claim-1",
        relation: "supports",
        statement: "Fragile thesis",
        origin: "user",
        contribution_status: "proposed",
        created_at: "2026-01-01T00:00:00Z",
        author: { id: "u1", display_name: null },
        sources: [],
        evidence: [],
      },
    });
    render(
      <ClaimDiscussion
        claimId="claim-1"
        commentCount={1}
        bibliography={[
          {
            id: "doc-1",
            title: "HAL article",
            document_type: "article",
            source_kind: "hal",
            external_id: "hal-1",
            academic_status: "unknown",
            epistemic: "bibliographic_resource",
            link: { relation: "about", score: 1 },
          },
        ]}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /open comments/i }));
    await waitFor(() => expect(screen.getByText("Fragile thesis")).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: "Promote to argument" }));
    fireEvent.change(screen.getByLabelText("Evidence excerpt"), { target: { value: "quote" } });
    fireEvent.click(screen.getByRole("button", { name: "Submit argument" }));
    await waitFor(() => expect(promoteCommentToArgument).toHaveBeenCalled());
    expect(screen.getByText("Fragile thesis")).toBeTruthy();
  });
});

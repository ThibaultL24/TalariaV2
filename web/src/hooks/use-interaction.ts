// web/src/hooks/use-interaction.ts
import { useCallback, useState } from "react";
import {
  deleteInteraction,
  postInteraction,
  type Interaction,
  type InteractionAction,
  type InteractionTargetType,
  type InteractionVisibility,
} from "@/lib/api";

export function useInteraction() {
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const upsert = useCallback(
    async (input: {
      action: InteractionAction;
      target_type: InteractionTargetType;
      target_id: string;
      visibility?: InteractionVisibility;
    }): Promise<Interaction | null> => {
      setIsSaving(true);
      setError(null);
      try {
        const result = await postInteraction(input);
        return result.interaction;
      } catch (err) {
        setError(err instanceof Error ? err.message : "request_failed");
        return null;
      } finally {
        setIsSaving(false);
      }
    },
    [],
  );

  const remove = useCallback(async (id: string): Promise<boolean> => {
    setIsSaving(true);
    setError(null);
    try {
      await deleteInteraction(id);
      return true;
    } catch (err) {
      setError(err instanceof Error ? err.message : "request_failed");
      return false;
    } finally {
      setIsSaving(false);
    }
  }, []);

  return { upsert, remove, isSaving, error };
}

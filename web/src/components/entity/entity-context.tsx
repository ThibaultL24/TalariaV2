// web/src/components/entity/entity-context.tsx
import { useEffect, useRef, useState } from "react";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import { usePersonPicker } from "@/hooks/use-person-picker";
import {
  defaultExperienceView,
  isExperienceView,
  type LifeExperience,
} from "@/lib/experiences";
import { useExplorerStore } from "@/stores/explorer-store";

export function useExperienceSubject(experience: LifeExperience) {
  const { entityId: routeEntityId, view: routeView } = useParams();
  const [params, setParams] = useSearchParams();
  const navigate = useNavigate();
  const storedId = useExplorerStore((state) => state.entityId);
  const pendingPerson = useExplorerStore((state) => state.personFilter);
  const entityId = routeEntityId ?? params.get("entity") ?? storedId ?? "";
  const view = routeView && isExperienceView(experience, routeView)
    ? routeView
    : defaultExperienceView(experience);
  const [revision, setRevision] = useState(0);
  const picker = usePersonPicker({
    onCountsChanged: () => setRevision((value) => value + 1),
  });
  const pendingSelection = useRef(false);
  const launchedPerson = useRef<string | undefined>(undefined);
  const landing = defaultExperienceView(experience);

  useEffect(() => {
    if (!routeEntityId) return;
    const missingView = !routeView;
    const unknownView = routeView != null && !isExperienceView(experience, routeView);
    if (!missingView && !unknownView) return;
    const query = params.toString();
    navigate(`/${experience}/${routeEntityId}/${landing}${query ? `?${query}` : ""}`, { replace: true });
  }, [experience, landing, navigate, params, routeEntityId, routeView]);

  useEffect(() => {
    if (routeEntityId || entityId || !pendingPerson) return;
    if (pendingSelection.current || launchedPerson.current === pendingPerson) return;
    launchedPerson.current = pendingPerson;
    pendingSelection.current = true;
    picker.selectPerson({
      label: pendingPerson,
      qid: useExplorerStore.getState().entityQid,
      known_locally: false,
    });
  }, [routeEntityId, entityId, pendingPerson, picker.selectPerson]);

  useEffect(() => {
    if (storedId && pendingSelection.current) {
      pendingSelection.current = false;
      navigate(`/${experience}/${storedId}/${landing}`);
      return;
    }
    if (!routeEntityId && entityId) {
      navigate(`/${experience}/${entityId}/${landing}`, { replace: true });
    }
  }, [storedId, entityId, routeEntityId, navigate, experience, landing]);

  const openPerson = (id: string) => {
    pendingSelection.current = false;
    navigate(`/${experience}/${id}/${landing}`);
  };

  return {
    entityId,
    routeEntityId,
    view,
    params,
    setParams,
    navigate,
    picker,
    revision,
    pendingSelection,
    openPerson,
  };
}

/** Keep the shared person in the explorer store without wiping filters when the id is unchanged. */
export function rememberEntity(entityId: string, label?: string, qid?: string | null) {
  const current = useExplorerStore.getState();
  if (current.entityId !== entityId) {
    current.setEntity(entityId, label, qid);
    return;
  }
  if (!label) return;
  if (current.entityLabel === label && (qid == null || current.entityQid === qid)) return;
  useExplorerStore.setState({
    entityLabel: label,
    entityQid: qid ?? current.entityQid,
  });
}

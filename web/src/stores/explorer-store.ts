// web/src/stores/explorer-store.ts
import { create } from "zustand";

export interface ExplorerFilters {
  /** Empty = show all types */
  types: string[];
  /** Empty = show all epistemic statuses */
  statuses: string[];
  /** Single selected profile slug, or undefined = all */
  profileSlug?: string;
  /** Single selected period slug, or undefined = all */
  periodSlug?: string;
  from?: string;
  to?: string;
  minConfidence?: number;
}

interface ExplorerState {
  entityId?: string;
  entityLabel?: string;
  personFilter?: string;
  selectedEventId?: string;
  hoveredEventId?: string;
  filters: ExplorerFilters;
  setEntity: (entityId?: string, entityLabel?: string) => void;
  setPersonFilter: (person?: string, entityLabel?: string) => void;
  setSelectedEventId: (eventId?: string) => void;
  setHoveredEventId: (eventId?: string) => void;
  setFilters: (patch: Partial<ExplorerFilters>) => void;
  toggleTypeFilter: (type: string) => void;
  toggleStatusFilter: (status: string) => void;
  setProfileFilter: (slug?: string) => void;
  setPeriodFilter: (slug?: string) => void;
  clearFilters: () => void;
  closeDetail: () => void;
  clearEntity: () => void;
}

const DEFAULT_FILTERS: ExplorerFilters = { types: [], statuses: [] };

function toggleInList(list: string[], value: string): string[] {
  return list.includes(value) ? list.filter((item) => item !== value) : [...list, value];
}

export const useExplorerStore = create<ExplorerState>((set) => ({
  entityId: undefined,
  entityLabel: undefined,
  personFilter: undefined,
  selectedEventId: undefined,
  hoveredEventId: undefined,
  filters: DEFAULT_FILTERS,
  setEntity: (entityId, entityLabel) =>
    set({
      entityId,
      entityLabel,
      personFilter: undefined,
      selectedEventId: undefined,
      filters: DEFAULT_FILTERS,
    }),
  setPersonFilter: (personFilter, entityLabel) =>
    set({
      personFilter,
      entityLabel,
      entityId: undefined,
      selectedEventId: undefined,
      filters: DEFAULT_FILTERS,
    }),
  setSelectedEventId: (selectedEventId) => set({ selectedEventId }),
  setHoveredEventId: (hoveredEventId) => set({ hoveredEventId }),
  setFilters: (patch) => set((state) => ({ filters: { ...state.filters, ...patch } })),
  toggleTypeFilter: (type) =>
    set((state) => ({
      filters: { ...state.filters, types: toggleInList(state.filters.types, type) },
    })),
  toggleStatusFilter: (status) =>
    set((state) => ({
      filters: { ...state.filters, statuses: toggleInList(state.filters.statuses, status) },
    })),
  setProfileFilter: (profileSlug) =>
    set((state) => ({
      filters: {
        ...state.filters,
        profileSlug: state.filters.profileSlug === profileSlug ? undefined : profileSlug,
      },
    })),
  setPeriodFilter: (periodSlug) =>
    set((state) => ({
      filters: {
        ...state.filters,
        periodSlug: state.filters.periodSlug === periodSlug ? undefined : periodSlug,
      },
    })),
  clearFilters: () =>
    set((state) => ({
      filters: { ...state.filters, types: [], statuses: [], profileSlug: undefined, periodSlug: undefined },
    })),
  closeDetail: () => set({ selectedEventId: undefined }),
  clearEntity: () =>
    set({
      entityId: undefined,
      entityLabel: undefined,
      personFilter: undefined,
      selectedEventId: undefined,
      filters: DEFAULT_FILTERS,
    }),
}));

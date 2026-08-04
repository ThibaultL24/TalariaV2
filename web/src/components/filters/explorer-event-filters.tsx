// web/src/components/filters/explorer-event-filters.tsx
import {
  EPISTEMIC_STATUS_OPTIONS,
  EVENT_TYPE_OPTIONS,
  eventTypeLabel,
  epistemicStatusLabel,
} from "@/lib/event-taxonomy";
import type { PeriodFacet, ProfileFacet } from "@/lib/schemas/entity";

interface ExplorerEventFiltersProps {
  availableTypes: string[];
  availableStatuses: string[];
  selectedTypes: string[];
  selectedStatuses: string[];
  profiles?: ProfileFacet[];
  periods?: PeriodFacet[];
  selectedProfileSlug?: string;
  selectedPeriodSlug?: string;
  onToggleType: (type: string) => void;
  onToggleStatus: (status: string) => void;
  onToggleProfile?: (slug: string) => void;
  onTogglePeriod?: (slug: string) => void;
  onClear: () => void;
}

export function ExplorerEventFilters({
  availableTypes,
  availableStatuses,
  selectedTypes,
  selectedStatuses,
  profiles = [],
  periods = [],
  selectedProfileSlug,
  selectedPeriodSlug,
  onToggleType,
  onToggleStatus,
  onToggleProfile,
  onTogglePeriod,
  onClear,
}: ExplorerEventFiltersProps) {
  const typeKeys =
    availableTypes.length > 0
      ? availableTypes
      : EVENT_TYPE_OPTIONS.map((option) => option.key);
  const statusKeys =
    availableStatuses.length > 0
      ? availableStatuses
      : EPISTEMIC_STATUS_OPTIONS.map((option) => option.key);

  const hasFilter =
    selectedTypes.length > 0 ||
    selectedStatuses.length > 0 ||
    Boolean(selectedProfileSlug) ||
    Boolean(selectedPeriodSlug);

  return (
    <div className="space-y-3 border-b border-(--color-border-subtle) px-3 py-3">
      <div className="flex items-center justify-between gap-2">
        <p className="text-[10px] font-semibold uppercase tracking-wide text-(--color-text-secondary)">
          Filters
        </p>
        {hasFilter ? (
          <button
            type="button"
            onClick={onClear}
            className="text-[10px] text-(--color-accent) hover:underline"
          >
            Show all
          </button>
        ) : (
          <span className="text-[10px] text-(--color-text-muted)">All visible</span>
        )}
      </div>

      {profiles.length > 0 && onToggleProfile ? (
        <div>
          <p className="mb-1.5 text-[10px] text-(--color-text-muted)">Profile</p>
          <div className="flex flex-wrap gap-1.5">
            {profiles.map((profile) => {
              const active = !selectedProfileSlug || selectedProfileSlug === profile.slug;
              const dimmed = Boolean(selectedProfileSlug) && selectedProfileSlug !== profile.slug;
              return (
                <button
                  key={profile.slug}
                  type="button"
                  onClick={() => onToggleProfile(profile.slug)}
                  className={`rounded-md border px-2 py-0.5 text-[10px] transition-colors ${
                    active && !dimmed
                      ? "border-(--color-accent) bg-(--color-accent)/15 text-(--color-text-primary)"
                      : "border-(--color-border-subtle) text-(--color-text-muted) opacity-50"
                  }`}
                >
                  {profile.label}
                </button>
              );
            })}
          </div>
        </div>
      ) : null}

      {periods.length > 0 && onTogglePeriod ? (
        <div>
          <p className="mb-1.5 text-[10px] text-(--color-text-muted)">Period</p>
          <div className="flex flex-wrap gap-1.5">
            {periods.map((period) => {
              const active = !selectedPeriodSlug || selectedPeriodSlug === period.slug;
              const dimmed = Boolean(selectedPeriodSlug) && selectedPeriodSlug !== period.slug;
              return (
                <button
                  key={period.slug}
                  type="button"
                  onClick={() => onTogglePeriod(period.slug)}
                  className={`rounded-md border px-2 py-0.5 text-[10px] transition-colors ${
                    active && !dimmed
                      ? "border-(--color-accent) bg-(--color-accent)/15 text-(--color-text-primary)"
                      : "border-(--color-border-subtle) text-(--color-text-muted) opacity-50"
                  }`}
                >
                  {period.label}
                </button>
              );
            })}
          </div>
        </div>
      ) : null}

      <div>
        <p className="mb-1.5 text-[10px] text-(--color-text-muted)">Category</p>
        <div className="flex flex-wrap gap-1.5">
          {typeKeys.map((type) => {
            const active = selectedTypes.length === 0 || selectedTypes.includes(type);
            const dimmed = selectedTypes.length > 0 && !selectedTypes.includes(type);
            return (
              <button
                key={type}
                type="button"
                onClick={() => onToggleType(type)}
                className={`rounded-md border px-2 py-0.5 text-[10px] transition-colors ${
                  active && !dimmed
                    ? "border-(--color-accent) bg-(--color-accent)/15 text-(--color-text-primary)"
                    : "border-(--color-border-subtle) text-(--color-text-muted) opacity-50"
                }`}
              >
                {eventTypeLabel(type)}
              </button>
            );
          })}
        </div>
      </div>

      <div>
        <p className="mb-1.5 text-[10px] text-(--color-text-muted)">Veracity</p>
        <div className="flex flex-wrap gap-1.5">
          {statusKeys.map((status) => {
            const active = selectedStatuses.length === 0 || selectedStatuses.includes(status);
            const dimmed = selectedStatuses.length > 0 && !selectedStatuses.includes(status);
            return (
              <button
                key={status}
                type="button"
                onClick={() => onToggleStatus(status)}
                className={`rounded-md border px-2 py-0.5 text-[10px] transition-colors ${
                  active && !dimmed
                    ? "border-(--color-accent) bg-(--color-accent)/15 text-(--color-text-primary)"
                    : "border-(--color-border-subtle) text-(--color-text-muted) opacity-50"
                }`}
              >
                {epistemicStatusLabel(status)}
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}

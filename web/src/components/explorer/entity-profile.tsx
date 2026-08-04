// web/src/components/explorer/entity-profile.tsx
interface EntityProfileProps {
  name: string;
  qid?: string | null;
  eventCount?: number;
  profiles?: Array<{ slug: string; label: string }>;
}

export function EntityProfile({ name, qid, eventCount, profiles = [] }: EntityProfileProps) {
  return (
    <div className="border-b border-(--color-border-subtle) px-4 py-3">
      <p className="text-[10px] font-semibold uppercase tracking-wide text-(--color-text-secondary)">
        Entity
      </p>
      <h2 className="mt-1 text-lg font-semibold leading-snug">{name}</h2>
      <div className="mt-1 flex flex-wrap gap-2 text-[11px] text-(--color-text-muted)">
        {qid ? <span>{qid}</span> : null}
        {eventCount != null ? <span>{eventCount} events</span> : null}
      </div>
      {profiles.length > 0 ? (
        <div className="mt-2 flex flex-wrap gap-1.5">
          {profiles.map((profile) => (
            <span
              key={profile.slug}
              className="rounded-md border border-(--color-border-subtle) bg-(--color-bg-primary)/40 px-2 py-0.5 text-[10px] text-(--color-text-secondary)"
            >
              {profile.label}
            </span>
          ))}
        </div>
      ) : null}
    </div>
  );
}

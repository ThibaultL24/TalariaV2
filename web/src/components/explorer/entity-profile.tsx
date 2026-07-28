// web/src/components/explorer/entity-profile.tsx
interface EntityProfileProps {
  name: string;
  qid?: string | null;
  eventCount?: number;
}

export function EntityProfile({ name, qid, eventCount }: EntityProfileProps) {
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
    </div>
  );
}

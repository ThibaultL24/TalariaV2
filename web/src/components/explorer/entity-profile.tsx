// web/src/components/explorer/entity-profile.tsx
interface EntityProfileProps {
  name: string;
  qid?: string | null;
  eventCount?: number;
  mapCount?: number;
  agoraCount?: number;
  bibliographyCount?: number;
  profiles?: Array<{ slug: string; label: string }>;
}

export function EntityProfile({
  name,
  qid,
  eventCount,
  mapCount,
  agoraCount,
  bibliographyCount,
  profiles = [],
}: EntityProfileProps) {
  return (
    <div className="border-b border-(--color-border-subtle) px-4 py-3">
      <h2 className="text-lg font-semibold leading-snug">{name}</h2>
      {qid ? <p className="mt-0.5 font-mono text-[10px] text-(--color-text-muted)">{qid}</p> : null}
      <div className="mt-1 flex flex-wrap gap-2 text-[11px] text-(--color-text-muted)">
        {eventCount != null ? <span>{eventCount} facts</span> : null}
        {mapCount != null ? <span>{mapCount} mapped</span> : null}
        {agoraCount != null && agoraCount > 0 ? (
          <span className="text-amber-200/80">{agoraCount} agora</span>
        ) : null}
        {bibliographyCount != null && bibliographyCount > 0 ? (
          <span>{bibliographyCount} sources</span>
        ) : null}
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

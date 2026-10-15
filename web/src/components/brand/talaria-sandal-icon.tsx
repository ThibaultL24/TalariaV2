// web/src/components/brand/talaria-sandal-icon.tsx
interface TalariaSandalIconProps {
  className?: string;
  title?: string;
}

/** Winged sandal (talaria) — line emblem for chrome and hero. */
export function TalariaSandalIcon({ className, title }: TalariaSandalIconProps) {
  return (
    <svg
      className={className}
      viewBox="0 0 64 40"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden={title ? undefined : true}
      role={title ? "img" : undefined}
    >
      {title ? <title>{title}</title> : null}
      <path
        d="M8 28 C14 32 28 34 44 30 C52 28 56 26 58 24"
        stroke="currentColor"
        strokeWidth="2.2"
        strokeLinecap="round"
      />
      <path
        d="M18 26 C22 18 30 14 38 16"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
      <path
        d="M24 28 C28 22 34 20 40 22"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
      <path
        d="M6 26 C2 20 0 12 4 6 C10 8 16 12 20 18 C16 22 12 26 8 28 C7 27 6 26 6 26 Z"
        fill="currentColor"
      />
      <path
        d="M8 28 C6 24 6 18 8 14 C12 16 14 20 14 24 C11 27 9 28 8 28 Z"
        fill="currentColor"
        opacity="0.75"
      />
      <path
        d="M10 28 C9 25 10 21 12 18 C14 20 15 23 14 26 C12 28 11 28 10 28 Z"
        fill="currentColor"
        opacity="0.55"
      />
    </svg>
  );
}

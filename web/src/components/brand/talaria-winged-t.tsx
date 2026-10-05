// web/src/components/brand/talaria-winged-t.tsx
interface TalariaWingedTProps {
  className?: string;
  title?: string;
}

/** Stylised winged « T » — brand glyph (original vector, not mockup assets). */
export function TalariaWingedT({ className, title }: TalariaWingedTProps) {
  return (
    <svg
      className={className}
      viewBox="0 0 40 48"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden={title ? undefined : true}
      role={title ? "img" : undefined}
    >
      {title ? <title>{title}</title> : null}
      <g transform="skewX(-10)">
        <path
          d="M22 6 H32 V10 H24 V42 H20 V10 H12 V6 H22 Z"
          fill="currentColor"
        />
      </g>
      <path
        d="M2 18 C0 12 1 6 5 2 C11 5 16 10 19 16 C15 20 11 24 7 27 C4 24 2 21 2 18 Z"
        fill="currentColor"
      />
      <path
        d="M7 27 C5 23 5 18 7 14 C11 16 14 19 15 23 C12 26 9 27 7 27 Z"
        fill="currentColor"
        opacity="0.82"
      />
      <path
        d="M9 28 C8 25 9 21 11 18 C13 20 14 23 13 26 C11 28 10 28 9 28 Z"
        fill="currentColor"
        opacity="0.6"
      />
    </svg>
  );
}

// web/src/components/brand/licensed-hero-video.tsx
import { useEffect, useRef, useState } from "react";
import { BRAND_HERO_DAY, BRAND_HERO_NIGHT } from "@/components/brand/brand-assets";
import { useI18n } from "@/lib/i18n";

/** A local file is required. Remote or stock previews are never accepted here. */
export function licensedHeroVideoSource(): string | undefined {
  if (import.meta.env.VITE_TALARIA_LICENSED_HERO_VIDEO !== "true") return undefined;
  const src = (import.meta.env.VITE_TALARIA_LICENSED_HERO_VIDEO_SRC ?? "").trim();
  if (!src.startsWith("/") || src.includes("..") || src.includes("://")) return undefined;
  return src;
}

function prefersStillImage(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return true;
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const connection = (navigator as Navigator & { connection?: { saveData?: boolean } }).connection;
  return reduced || Boolean(connection?.saveData);
}

export function LicensedHeroVideo() {
  const src = licensedHeroVideoSource();
  const { locale } = useI18n();
  const videoRef = useRef<HTMLVideoElement>(null);
  const [failed, setFailed] = useState(false);
  const [playing, setPlaying] = useState(false);
  const still = prefersStillImage();
  const night = typeof document !== "undefined" && document.documentElement.dataset.theme === "dark";

  useEffect(() => {
    const video = videoRef.current;
    if (!video || !src || failed || still) return;
    void video.play().then(() => setPlaying(true)).catch(() => setFailed(true));
  }, [src, failed, still]);

  if (!src || failed) return null;

  const pauseLabel = locale === "fr" ? "Mettre la vidéo en pause" : "Pause the background video";
  const playLabel = locale === "fr" ? "Reprendre la vidéo" : "Play the background video";

  return (
    <>
      <video
        ref={videoRef}
        src={src}
        poster={night ? BRAND_HERO_NIGHT : BRAND_HERO_DAY}
        muted
        playsInline
        loop
        preload={still ? "none" : "metadata"}
        onError={() => setFailed(true)}
        style={{ position: "absolute", inset: 0, width: "100%", height: "100%", objectFit: "cover", zIndex: 0 }}
      />
      <div
        aria-hidden
        style={{
          position: "absolute",
          inset: 0,
          zIndex: 0,
          pointerEvents: "none",
          background: "linear-gradient(180deg, rgba(8, 16, 28, 0.2), rgba(8, 16, 28, 0.62))",
        }}
      />
      <button
        type="button"
        className="button button--ghost"
        aria-pressed={playing}
        style={{ position: "absolute", right: "1rem", bottom: "1rem", zIndex: 2 }}
        onClick={() => {
          const video = videoRef.current;
          if (!video) return;
          if (video.paused) {
            void video.play().then(() => setPlaying(true)).catch(() => setFailed(true));
            return;
          }
          video.pause();
          setPlaying(false);
        }}
      >
        {playing ? pauseLabel : playLabel}
      </button>
    </>
  );
}

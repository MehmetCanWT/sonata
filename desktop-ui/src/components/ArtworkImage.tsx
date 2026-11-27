import { Disc3 } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { resolveArtworkSrc } from "@/lib/ipc";
import { cn } from "@/lib/utils";

const MAX_MOTION_BLOB_CACHE = 2;
const motionBlobCache = new Map<string, string>();

export function evictInactiveMotionBlobs(keepKey?: string) {
  for (const [key, blobUrl] of motionBlobCache.entries()) {
    if (key === keepKey) continue;
    motionBlobCache.delete(key);
    URL.revokeObjectURL(blobUrl);
  }
}

export function getMotionBlobCacheSizeForTest(): number {
  return motionBlobCache.size;
}

function getCachedMotionBlob(key: string): string | undefined {
  const existing = motionBlobCache.get(key);
  if (!existing) return undefined;
  // Refresh LRU order.
  motionBlobCache.delete(key);
  motionBlobCache.set(key, existing);
  return existing;
}

function setCachedMotionBlob(key: string, blobUrl: string) {
  const existing = motionBlobCache.get(key);
  if (existing && existing !== blobUrl) {
    URL.revokeObjectURL(existing);
  }
  motionBlobCache.delete(key);
  motionBlobCache.set(key, blobUrl);
  while (motionBlobCache.size > MAX_MOTION_BLOB_CACHE) {
    const oldestKey = motionBlobCache.keys().next().value;
    if (!oldestKey) break;
    const oldestVal = motionBlobCache.get(oldestKey);
    motionBlobCache.delete(oldestKey);
    if (oldestVal) {
      URL.revokeObjectURL(oldestVal);
    }
  }
}

interface ArtworkImageProps {
  src?: string | null;
  motionSrc?: string | null;
  motionPaused?: boolean;
  alt: string;
  className?: string;
  imgClassName?: string;
  rounded?: string;
}

export function ArtworkImage({
  src,
  motionSrc,
  motionPaused = false,
  alt,
  className,
  imgClassName,
  rounded = "rounded-lg",
}: ArtworkImageProps) {
  const [failed, setFailed] = useState(false);
  const directMotionUrl = resolveArtworkSrc(motionSrc);
  const initialCached = directMotionUrl ? getCachedMotionBlob(directMotionUrl) : undefined;
  const [videoSrc, setVideoSrc] = useState<string | undefined>(initialCached ?? directMotionUrl);
  const [videoReady, setVideoReady] = useState(false);
  const [usedBlobFallback, setUsedBlobFallback] = useState(Boolean(initialCached));
  const [videoFailed, setVideoFailed] = useState(false);
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const stallTimerRef = useRef<number | null>(null);

  useEffect(() => {
    setFailed(false);
  }, [src]);

  useEffect(() => {
    if (stallTimerRef.current !== null) {
      window.clearTimeout(stallTimerRef.current);
      stallTimerRef.current = null;
    }
    if (!directMotionUrl) {
      setVideoSrc(undefined);
      setVideoReady(false);
      setUsedBlobFallback(false);
      setVideoFailed(false);
      return;
    }
    const cached = getCachedMotionBlob(directMotionUrl);
    if (cached) {
      setVideoSrc(cached);
      setVideoReady(false);
      setUsedBlobFallback(true);
      setVideoFailed(false);
      return;
    }

    // Eagerly preload into a RAM blob URL so seamless looping never stalls on Tauri asset IPC.
    let cancelled = false;
    const controller = typeof AbortController !== "undefined" ? new AbortController() : null;
    setVideoSrc(undefined);
    setVideoReady(false);
    setUsedBlobFallback(true);
    setVideoFailed(false);

    if (typeof fetch === "function") {
      void fetch(
        directMotionUrl,
        controller ? { signal: controller.signal } : undefined,
      )
        .then((res) => {
          if (!res.ok) throw new Error(`Motion artwork HTTP ${res.status}`);
          return res.arrayBuffer();
        })
        .then((buf) => {
          if (cancelled) return;
          const blobUrl = URL.createObjectURL(new Blob([buf], { type: "video/mp4" }));
          setCachedMotionBlob(directMotionUrl, blobUrl);
          setVideoSrc(blobUrl);
        })
        .catch(() => {
          if (cancelled) return;
          setUsedBlobFallback(false);
          setVideoSrc(directMotionUrl);
        });
    } else {
      setUsedBlobFallback(false);
      setVideoSrc(directMotionUrl);
    }

    return () => {
      cancelled = true;
      controller?.abort();
      if (stallTimerRef.current !== null) {
        window.clearTimeout(stallTimerRef.current);
        stallTimerRef.current = null;
      }
    };
  }, [directMotionUrl]);

  const ensurePlaying = useCallback(() => {
    const el = videoRef.current;
    if (!el || !videoSrc || videoFailed || motionPaused) return;
    if (el.readyState >= 2 && el.paused) {
      void el.play().catch(() => {});
    }
  }, [motionPaused, videoFailed, videoSrc]);

  useEffect(() => {
    const el = videoRef.current;
    if (!el || !videoSrc || videoFailed) return;
    if (motionPaused) {
      el.pause();
    } else {
      void el.play().catch(() => {});
    }
  }, [motionPaused, videoSrc, videoFailed]);

  useEffect(() => {
    if (!videoSrc || videoFailed || motionPaused) return;
    const onResume = () => {
      if (document.visibilityState === "hidden") {
        videoRef.current?.pause();
        evictInactiveMotionBlobs(directMotionUrl);
      } else {
        ensurePlaying();
      }
    };
    window.addEventListener("focus", onResume);
    window.addEventListener("resize", onResume);
    document.addEventListener("visibilitychange", onResume);
    return () => {
      window.removeEventListener("focus", onResume);
      window.removeEventListener("resize", onResume);
      document.removeEventListener("visibilitychange", onResume);
    };
  }, [directMotionUrl, ensurePlaying, motionPaused, videoFailed, videoSrc]);

  const scheduleRecovery = useCallback(() => {
    if (motionPaused || videoFailed) return;
    if (stallTimerRef.current !== null) {
      window.clearTimeout(stallTimerRef.current);
    }
    stallTimerRef.current = window.setTimeout(() => {
      stallTimerRef.current = null;
      ensurePlaying();
    }, 120);
  }, [ensurePlaying, motionPaused, videoFailed]);

  const resolved = failed ? undefined : resolveArtworkSrc(src);

  const handleVideoError = () => {
    if (usedBlobFallback || !directMotionUrl) {
      setVideoFailed(true);
      setVideoReady(false);
      return;
    }
    setUsedBlobFallback(true);
    setVideoReady(false);
    void fetch(directMotionUrl)
      .then((res) => {
        if (!res.ok) throw new Error(`Motion artwork HTTP ${res.status}`);
        return res.arrayBuffer();
      })
      .then((buf) => {
        const blobUrl = URL.createObjectURL(new Blob([buf], { type: "video/mp4" }));
        setCachedMotionBlob(directMotionUrl, blobUrl);
        setVideoSrc(blobUrl);
      })
      .catch(() => {
        setVideoFailed(true);
      });
  };

  return (
    <div
      className={cn(
        "relative shrink-0 overflow-hidden bg-[var(--bg-elevated)] border border-[var(--border-hairline)] flex items-center justify-center",
        rounded,
        className,
      )}
    >
      {resolved ? (
        <img
          src={resolved}
          alt={alt}
          loading="lazy"
          decoding="async"
          onError={() => setFailed(true)}
          className={cn("h-full w-full object-cover", imgClassName)}
        />
      ) : (
        <Disc3 className="h-2/5 w-2/5 text-[var(--ink-muted)]" />
      )}

      {videoSrc && !videoFailed && (
        <video
          ref={videoRef}
          src={videoSrc}
          autoPlay={!motionPaused}
          loop
          muted
          playsInline
          preload="auto"
          onCanPlay={(e) => {
            setVideoReady(true);
            if (motionPaused) {
              e.currentTarget.pause();
            } else {
              void e.currentTarget.play().catch(() => {});
            }
          }}
          onEnded={(e) => {
            const el = e.currentTarget;
            el.currentTime = el.seekable.length > 0 ? el.seekable.start(0) : 0;
            if (!motionPaused) {
              void el.play().catch(() => {});
            }
          }}
          onStalled={scheduleRecovery}
          onPause={() => {
            const el = videoRef.current;
            if (!motionPaused && el && !el.ended && el.readyState >= 2) {
              scheduleRecovery();
            }
          }}
          onError={handleVideoError}
          className={cn(
            "pointer-events-none absolute inset-0 h-full w-full object-cover transition-opacity duration-300",
            videoReady ? "opacity-100" : "opacity-0",
            imgClassName,
          )}
        />
      )}
    </div>
  );
}

import { Pause, Play, Square, RotateCcw, RotateCw } from "lucide-react";
import { useEffect, useRef } from "react";

import { api, type PlayableRef } from "@/lib/api";
import { usePlayerStatus } from "@/lib/hooks";
import { Button } from "@/components/ui";

/** Formats seconds as h:mm:ss, or m:ss under an hour. */
function clock(secs: number) {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = Math.floor(secs % 60);
  const mm = h > 0 ? String(m).padStart(2, "0") : String(m);
  return `${h > 0 ? `${h}:` : ""}${mm}:${String(s).padStart(2, "0")}`;
}

export function PlayerBar({ nowPlaying }: { nowPlaying: PlayableRef | null }) {
  const { data } = usePlayerStatus(true);
  const lastSaved = useRef(0);

  // Persist the Resume Point as it moves. Rust drops it for Channels and for
  // anything under 30s or over 95%, so this can stay dumb.
  useEffect(() => {
    if (!nowPlaying || !data?.playing || data.positionSecs == null) return;
    const pos = data.positionSecs;
    if (Math.abs(pos - lastSaved.current) < 10) return;
    lastSaved.current = pos;
    void api.saveResumePoint(nowPlaying, pos, data.durationSecs).catch(() => {
      // A failed Resume Point write is not worth interrupting playback for.
    });
  }, [data?.positionSecs, data?.playing, data?.durationSecs, nowPlaying]);

  if (!data?.running) return null;

  const pos = data.positionSecs ?? 0;
  const dur = data.durationSecs ?? 0;
  const isLive = dur <= 0;
  const pct = isLive ? 0 : Math.min(100, (pos / dur) * 100);

  return (
    <div className="flex h-16 shrink-0 items-center gap-4 border-t border-[var(--border)] bg-[var(--card)] px-4">
      <div className="flex items-center gap-1">
        <Button size="sm" variant="ghost" onClick={() => void api.seek(-30)}>
          <RotateCcw className="size-4" />
        </Button>
        <Button size="sm" variant="ghost" onClick={() => void api.togglePause()}>
          {data.paused ? (
            <Play className="size-4" />
          ) : (
            <Pause className="size-4" />
          )}
        </Button>
        <Button size="sm" variant="ghost" onClick={() => void api.seek(30)}>
          <RotateCw className="size-4" />
        </Button>
      </div>

      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-medium">
          {data.title ?? "Playing"}
        </div>
        {isLive ? (
          <div className="mt-1 flex items-center gap-2 text-xs text-[var(--muted-foreground)]">
            <span className="size-1.5 rounded-full bg-[var(--destructive)]" />
            Live
          </div>
        ) : (
          <div className="mt-1 flex items-center gap-2">
            <div className="h-1 flex-1 overflow-hidden rounded-full bg-[var(--muted)]">
              <div
                className="h-full bg-[var(--primary)]"
                style={{ width: `${pct}%` }}
              />
            </div>
            <span className="tabular-nums text-xs text-[var(--muted-foreground)]">
              {clock(pos)} / {clock(dur)}
            </span>
          </div>
        )}
      </div>

      <Button size="sm" variant="ghost" onClick={() => void api.stop()}>
        <Square className="size-4" />
      </Button>
    </div>
  );
}

import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/** Seconds since epoch to something a human reads at a glance. */
export function formatDate(ts: number | null | undefined) {
  if (!ts) return "never";
  return new Date(ts * 1000).toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

export function formatClock(ts: number) {
  return new Date(ts * 1000).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function formatDuration(secs: number | null | undefined) {
  if (secs == null || secs <= 0) return null;
  const h = Math.floor(secs / 3600);
  const m = Math.round((secs % 3600) / 60);
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

/** How stale the mirror is, in whole days. */
export function daysSince(ts: number | null | undefined) {
  if (!ts) return null;
  return Math.floor((Date.now() / 1000 - ts) / 86400);
}

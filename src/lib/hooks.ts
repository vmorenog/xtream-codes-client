import { useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";

import { api, type Provider } from "@/lib/api";

/** Which **Provider** the UI is currently showing. Survives a restart. */
const ACTIVE_PROVIDER_KEY = "xtream.activeProviderId";

export function useProviders() {
  return useQuery({ queryKey: ["providers"], queryFn: api.providers });
}

export function useMpvInstalled() {
  return useQuery({
    queryKey: ["mpv"],
    queryFn: api.mpvInstalled,
    // Installing mpv happens outside the app; a stale "missing" would strand
    // the user on the setup screen until they restart.
    staleTime: 10_000,
  });
}

export function useActiveProvider(providers: Provider[] | undefined) {
  const [storedId, setStoredId] = useState<number | null>(() => {
    const raw = localStorage.getItem(ACTIVE_PROVIDER_KEY);
    return raw ? Number(raw) : null;
  });

  const setActive = useCallback((id: number) => {
    localStorage.setItem(ACTIVE_PROVIDER_KEY, String(id));
    setStoredId(id);
  }, []);

  // A stored id can point at a Provider that has since been removed.
  const active =
    providers?.find((p) => p.id === storedId) ?? providers?.[0] ?? null;

  useEffect(() => {
    if (active && active.id !== storedId) setActive(active.id);
  }, [active, storedId, setActive]);

  return { active, setActive };
}

/**
 * Polls mpv for playback position.
 *
 * Polling rather than a subscription: mpv's IPC socket interleaves replies with
 * its own event stream, and one-second polling is plenty for a progress bar
 * without having to demultiplex that.
 */
export function usePlayerStatus(enabled: boolean) {
  return useQuery({
    queryKey: ["player"],
    queryFn: api.playerStatus,
    refetchInterval: enabled ? 1000 : false,
    enabled,
  });
}

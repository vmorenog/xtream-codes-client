import { useCallback, useState } from "react";

import { api, errorMessage, type PlayableRef } from "@/lib/api";
import { AppContext } from "@/lib/app-context";
import { useActiveProvider, useMpvInstalled, useProviders } from "@/lib/hooks";
import { AppShell } from "@/components/AppShell";
import { FirstRun, MpvMissing } from "@/components/Setup";
import { Notice } from "@/components/ui";

/**
 * Decides which of the three states the app is in before any route renders:
 * mpv missing, no **Provider** yet, or ready.
 */
export function App() {
  const mpv = useMpvInstalled();
  const providers = useProviders();
  const { active, setActive } = useActiveProvider(providers.data);
  const [nowPlaying, setNowPlaying] = useState<PlayableRef | null>(null);
  const [playError, setPlayError] = useState<string | null>(null);

  const play = useCallback((ref: PlayableRef) => {
    setPlayError(null);
    setNowPlaying(ref);
    void api.play(ref).catch((e) => {
      setNowPlaying(null);
      setPlayError(errorMessage(e));
    });
  }, []);

  if (mpv.isPending || providers.isPending) return null;
  if (mpv.data === false) return <MpvMissing onRecheck={() => void mpv.refetch()} />;
  if (providers.error)
    return <Notice title="Could not open the catalogue">{errorMessage(providers.error)}</Notice>;
  if (!active) return <FirstRun />;

  return (
    <AppContext.Provider value={{ provider: active, play }}>
      {playError ? (
        <div className="bg-[var(--destructive)] px-4 py-2 text-sm text-[var(--destructive-foreground)]">
          {playError}
        </div>
      ) : null}
      <AppShell
        providers={providers.data}
        active={active}
        setActive={setActive}
        nowPlaying={nowPlaying}
        setNowPlaying={setNowPlaying}
      />
    </AppContext.Provider>
  );
}

import { listen } from "@tauri-apps/api/event";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link, Outlet, useNavigate } from "@tanstack/react-router";
import { Clapperboard, Film, RefreshCw, Search, Tv } from "lucide-react";
import { useEffect, useState } from "react";

import {
  api,
  errorMessage,
  type PlayableRef,
  type Provider,
  type SyncProgress,
} from "@/lib/api";
import { useApp } from "@/lib/app-context";
import { cn, daysSince, formatDate } from "@/lib/utils";
import { Button, Input, Muted } from "@/components/ui";
import { PlayerBar } from "@/components/PlayerBar";

const SECTIONS = [
  { to: "/live", label: "Live TV", icon: Tv },
  { to: "/movies", label: "Movies", icon: Film },
  { to: "/series", label: "Series", icon: Clapperboard },
] as const;

/** Nudge the Viewer once the mirror is this old (ADR-0004). */
const STALE_AFTER_DAYS = 7;

export function AppShell({
  providers,
  active,
  setActive,
  nowPlaying,
}: {
  providers: Provider[];
  active: Provider;
  setActive: (id: number) => void;
  nowPlaying: PlayableRef | null;
}) {
  return (
    <div className="flex h-full flex-col">
      <div className="flex min-h-0 flex-1">
        <Sidebar providers={providers} active={active} setActive={setActive} />
        <main className="flex min-w-0 flex-1 flex-col">
          <TopBar active={active} />
          <Outlet />
        </main>
      </div>
      <PlayerBar nowPlaying={nowPlaying} />
    </div>
  );
}

function Sidebar({
  providers,
  active,
  setActive,
}: {
  providers: Provider[];
  active: Provider;
  setActive: (id: number) => void;
}) {
  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-[var(--border)] bg-[var(--card)]">
      {/* Traffic lights float over this strip; keep it empty and draggable. */}
      <div className="titlebar-drag h-10 shrink-0" />

      <div className="px-3 pb-3">
        {providers.length > 1 ? (
          <select
            value={active.id}
            onChange={(e) => setActive(Number(e.target.value))}
            className="h-8 w-full rounded-md border border-[var(--input)] bg-[var(--background)] px-2 text-sm"
          >
            {providers.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        ) : (
          <p className="truncate text-sm font-medium">{active.name}</p>
        )}
      </div>

      <nav className="flex-1 space-y-0.5 px-2">
        {SECTIONS.map(({ to, label, icon: Icon }) => (
          <Link
            key={to}
            to={to}
            className="flex items-center gap-2.5 rounded-md px-2.5 py-1.5 text-sm hover:bg-[var(--accent)]"
            activeProps={{ className: "bg-[var(--accent)] font-medium" }}
          >
            <Icon className="size-4" />
            {label}
          </Link>
        ))}
      </nav>

      <SyncPanel provider={active} />
    </aside>
  );
}

function SyncPanel({ provider }: { provider: Provider }) {
  const qc = useQueryClient();
  const [progress, setProgress] = useState<SyncProgress | null>(null);

  useEffect(() => {
    const unlisten = listen<SyncProgress>("sync:progress", (e) => {
      if (e.payload.providerId === provider.id) setProgress(e.payload);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [provider.id]);

  const sync = useMutation({
    mutationFn: () => api.sync(provider.id),
    onSettled: async () => {
      setProgress(null);
      // Everything downstream of the mirror is now wrong.
      await qc.invalidateQueries();
    },
  });

  const stale = daysSince(provider.lastSyncedAt);
  const isStale = stale === null || stale >= STALE_AFTER_DAYS;

  return (
    <div className="space-y-2 border-t border-[var(--border)] p-3">
      {sync.error ? (
        <p className="text-xs text-[var(--destructive)]">
          {errorMessage(sync.error)}
        </p>
      ) : isStale ? (
        <Muted className="text-xs">
          {stale === null
            ? "Never synced."
            : `Catalogue is ${stale} days old.`}
        </Muted>
      ) : (
        <Muted className="text-xs">
          Synced {formatDate(provider.lastSyncedAt)}
        </Muted>
      )}

      <Button
        size="sm"
        variant={isStale ? "primary" : "outline"}
        className="w-full"
        disabled={sync.isPending}
        onClick={() => sync.mutate()}
      >
        <RefreshCw
          className={cn("size-3.5", sync.isPending && "animate-spin")}
        />
        {sync.isPending
          ? progress
            ? `${progress.stage} ${progress.items || ""}`.trim()
            : "Syncing…"
          : "Sync"}
      </Button>
    </div>
  );
}

function TopBar({ active }: { active: Provider }) {
  const { play } = useApp();
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<Awaited<ReturnType<typeof api.search>>>([]);
  const navigate = useNavigate();

  useEffect(() => {
    if (query.trim().length < 2) {
      setHits([]);
      return;
    }
    // FTS over 80k rows is sub-millisecond, but debouncing keeps keystrokes
    // from queueing up behind each other on the IPC bridge.
    const t = setTimeout(() => {
      void api.search(active.id, query).then(setHits).catch(() => setHits([]));
    }, 120);
    return () => clearTimeout(t);
  }, [query, active.id]);

  return (
    <div className="relative shrink-0 border-b border-[var(--border)]">
      <div className="titlebar-drag flex h-12 items-center gap-2 px-4">
        <Search className="size-4 shrink-0 text-[var(--muted-foreground)]" />
        <Input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search channels, movies and series"
          className="h-8 border-none bg-transparent px-0 focus-visible:ring-0"
          style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
        />
      </div>

      {hits.length > 0 ? (
        <ul className="absolute inset-x-0 top-full z-20 max-h-96 overflow-y-auto border-b border-[var(--border)] bg-[var(--popover)] shadow-xl">
          {hits.map((h) => (
            <li key={`${h.kind}-${h.refId}`}>
              <button
                className="flex w-full items-center gap-3 px-4 py-2 text-left text-sm hover:bg-[var(--accent)]"
                onClick={() => {
                  setQuery("");
                  setHits([]);
                  // A Series is not playable; it opens instead.
                  if (h.kind === "series") {
                    void navigate({
                      to: "/series/$seriesId",
                      params: { seriesId: h.refId },
                    });
                    return;
                  }
                  play({
                    providerId: active.id,
                    kind: h.kind,
                    refId: h.refId,
                  });
                }}
              >
                <span className="w-14 shrink-0 text-xs uppercase text-[var(--muted-foreground)]">
                  {h.kind}
                </span>
                <span className="truncate">{h.name}</span>
              </button>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}

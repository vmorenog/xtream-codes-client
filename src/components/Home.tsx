import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Check, Play, Star, Tv } from "lucide-react";

import {
  api,
  errorMessage,
  type ContinueItem,
  type PlayableRef,
} from "@/lib/api";
import { useApp } from "@/lib/app-context";
import { Muted, Notice } from "@/components/ui";

/**
 * The landing surface.
 *
 * Favourite Channels first, deliberately: on most nights the thing you want is
 * one of your dozen channels, immediately. The half-watched film is the
 * exception and sits one section down.
 */
export function Home() {
  const { provider, play } = useApp();

  const favourites = useQuery({
    queryKey: ["favourites", provider.id],
    queryFn: () => api.favourites(provider.id),
  });
  const continueWatching = useQuery({
    queryKey: ["continue", provider.id],
    queryFn: () => api.continueWatching(provider.id),
  });

  if (favourites.error)
    return <Notice title="Could not read Home">{errorMessage(favourites.error)}</Notice>;

  const favs = favourites.data;
  const cont = continueWatching.data ?? [];
  const empty =
    favs != null &&
    !favs.channels.length &&
    !favs.movies.length &&
    !favs.series.length &&
    !cont.length;

  if (favourites.isPending) return <Notice title="Loading…" />;

  if (empty)
    return (
      <Notice title="Nothing pinned yet">
        Star a channel or start a film and it turns up here. Live TV is in the
        sidebar.
      </Notice>
    );

  return (
    <div className="overflow-y-auto p-6">
      {favs && favs.channels.length > 0 ? (
        <Section title="Channels">
          <div className="grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-2">
            {favs.channels.map((c) => (
              <button
                key={c.streamId}
                onClick={() =>
                  play({
                    providerId: provider.id,
                    kind: "channel",
                    refId: String(c.streamId),
                  })
                }
                className="flex items-center gap-2.5 rounded-lg border border-[var(--border)] p-2.5 text-left hover:bg-[var(--accent)]"
              >
                {c.icon ? (
                  <img
                    src={c.icon}
                    alt=""
                    loading="lazy"
                    className="size-8 shrink-0 rounded object-contain"
                    onError={(e) => {
                      e.currentTarget.style.visibility = "hidden";
                    }}
                  />
                ) : (
                  <Tv className="size-8 shrink-0 p-1.5 text-[var(--muted-foreground)]" />
                )}
                <span className="min-w-0 flex-1 truncate text-sm">{c.name}</span>
              </button>
            ))}
          </div>
        </Section>
      ) : null}

      {cont.length > 0 ? (
        <Section title="Continue watching">
          <div className="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-4">
            {cont.map((item) => (
              <ContinueCard
                key={`${item.kind}-${item.refId}`}
                item={item}
                providerId={provider.id}
                onPlay={play}
              />
            ))}
          </div>
        </Section>
      ) : null}

      {favs && favs.series.length > 0 ? (
        <Section title="Series">
          <PosterGrid>
            {favs.series.map((s) => (
              <Link
                key={s.seriesId}
                to="/series/$seriesId"
                params={{ seriesId: String(s.seriesId) }}
                className="group"
              >
                <Poster src={s.cover} />
                <p className="mt-2 line-clamp-2 text-xs">{s.name}</p>
              </Link>
            ))}
          </PosterGrid>
        </Section>
      ) : null}

      {favs && favs.movies.length > 0 ? (
        <Section title="Movies">
          <PosterGrid>
            {favs.movies.map((m) => (
              <button
                key={m.streamId}
                className="group text-left"
                onClick={() =>
                  play({
                    providerId: provider.id,
                    kind: "movie",
                    refId: String(m.streamId),
                  })
                }
              >
                <Poster src={m.icon} />
                <p className="mt-2 line-clamp-2 text-xs">{m.name}</p>
              </button>
            ))}
          </PosterGrid>
        </Section>
      ) : null}
    </div>
  );
}

function Section({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section className="mb-9">
      <h2 className="mb-3 text-sm font-semibold">{title}</h2>
      {children}
    </section>
  );
}

function PosterGrid({ children }: { children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-4">
      {children}
    </div>
  );
}

function Poster({ src }: { src: string | null }) {
  return (
    <div className="aspect-[2/3] overflow-hidden rounded-lg bg-[var(--muted)]">
      {src ? (
        <img
          src={src}
          alt=""
          loading="lazy"
          className="size-full object-cover transition group-hover:scale-105"
          onError={(e) => {
            e.currentTarget.style.visibility = "hidden";
          }}
        />
      ) : null}
    </div>
  );
}

function ContinueCard({
  item,
  providerId,
  onPlay,
}: {
  item: ContinueItem;
  providerId: number;
  onPlay: (r: PlayableRef) => void;
}) {
  const qc = useQueryClient();
  const ref: PlayableRef = {
    providerId,
    kind: item.kind,
    refId: item.refId,
  };

  // The escape hatch for a deliberately skipped Episode, which Up Next would
  // otherwise keep offering forever (ADR-0006).
  const skip = useMutation({
    mutationFn: () => api.markWatched(ref),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["continue"] }),
  });

  const pct =
    item.positionSecs != null && item.durationSecs
      ? (item.positionSecs / item.durationSecs) * 100
      : null;

  const episodeLabel =
    item.season != null && item.episodeNumber != null
      ? `S${item.season}E${item.episodeNumber}`
      : null;

  return (
    <div className="group relative">
      <button className="w-full text-left" onClick={() => onPlay(ref)}>
        <div className="relative aspect-video overflow-hidden rounded-lg bg-[var(--muted)]">
          {item.icon ? (
            <img
              src={item.icon}
              alt=""
              loading="lazy"
              className="size-full object-cover transition group-hover:scale-105"
              onError={(e) => {
                e.currentTarget.style.visibility = "hidden";
              }}
            />
          ) : null}

          <div className="absolute inset-0 flex items-center justify-center opacity-0 transition group-hover:opacity-100">
            <span className="rounded-full bg-black/60 p-2.5">
              <Play className="size-5" />
            </span>
          </div>

          {pct != null ? (
            <div className="absolute inset-x-0 bottom-0 h-1 bg-black/50">
              <div
                className="h-full bg-[var(--primary)]"
                style={{ width: `${pct}%` }}
              />
            </div>
          ) : null}
        </div>

        <p className="mt-2 truncate text-xs font-medium">
          {item.seriesName ?? item.name}
        </p>
        <Muted className="truncate text-xs">
          {item.isUpNext ? "Up next · " : ""}
          {episodeLabel ? `${episodeLabel} · ${item.name}` : ""}
          {!episodeLabel && !item.isUpNext ? "Resume" : ""}
        </Muted>
      </button>

      <button
        title="Mark as watched"
        onClick={() => skip.mutate()}
        className="absolute right-1.5 top-1.5 rounded-md bg-black/60 p-1.5 opacity-0 transition group-hover:opacity-100"
      >
        <Check className="size-3.5" />
      </button>
    </div>
  );
}

/** Star toggle usable for a Channel, Movie, Episode or Series. */
export function FavouriteToggle({
  providerId,
  kind,
  refId,
  active,
}: {
  providerId: number;
  kind: "channel" | "movie" | "episode" | "series";
  refId: string;
  active?: boolean;
}) {
  const qc = useQueryClient();
  const toggle = useMutation({
    mutationFn: () => api.toggleFavourite({ providerId, kind, refId }),
    onSuccess: () => qc.invalidateQueries(),
  });
  return (
    <button
      title={active ? "Remove from favourites" : "Add to favourites"}
      onClick={(e) => {
        e.stopPropagation();
        e.preventDefault();
        toggle.mutate();
      }}
      className="rounded-md p-1.5 hover:bg-[var(--accent)]"
    >
      <Star
        className={
          active ? "size-4 fill-current text-[var(--primary)]" : "size-4"
        }
      />
    </button>
  );
}

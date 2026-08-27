import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Play, Star, Tv } from "lucide-react";
import { useState } from "react";

import {
  api,
  errorMessage,
  type CatalogueKind,
  type Category,
  type PlayableRef,
} from "@/lib/api";
import { cn, formatDuration } from "@/lib/utils";
import { Button, Muted, Notice } from "@/components/ui";
import { FavouriteToggle } from "@/components/Home";

interface Props {
  providerId: number;
  kind: CatalogueKind;
  onPlay: (ref: PlayableRef) => void;
}

/** Category rail plus the items in the selected category. */
export function Catalogue({ providerId, kind, onPlay }: Props) {
  const [categoryId, setCategoryId] = useState<number | null>(null);

  const categories = useQuery({
    queryKey: ["categories", providerId, kind],
    queryFn: () => api.categories(providerId, kind),
  });

  return (
    <div className="flex min-h-0 flex-1">
      <nav className="w-60 shrink-0 overflow-y-auto border-r border-[var(--border)] p-2">
        <CategoryButton
          active={categoryId === null}
          onClick={() => setCategoryId(null)}
          name="All"
        />
        {withRegionHeadings(categories.data ?? []).map((row) =>
          row.heading ? (
            <p
              key={`h-${row.regionCode}`}
              className="mt-4 mb-1 px-2.5 text-[10px] font-semibold uppercase tracking-wider text-[var(--muted-foreground)]"
            >
              {row.heading}
            </p>
          ) : (
            <CategoryButton
              key={row.category!.id}
              active={categoryId === row.category!.id}
              onClick={() => setCategoryId(row.category!.id)}
              name={row.category!.name}
              count={row.category!.count}
              star={
                <CategoryStar
                  providerId={providerId}
                  kind={kind}
                  categoryId={row.category!.id}
                  active={row.category!.isFavourite}
                />
              }
            />
          ),
        )}
      </nav>

      <div className="min-w-0 flex-1 overflow-y-auto">
        {kind === "live" ? (
          <Channels
            providerId={providerId}
            categoryId={categoryId}
            onPlay={onPlay}
          />
        ) : kind === "movie" ? (
          <Movies
            providerId={providerId}
            categoryId={categoryId}
            onPlay={onPlay}
          />
        ) : (
          <SeriesGrid providerId={providerId} categoryId={categoryId} />
        )}
      </div>
    </div>
  );
}

/**
 * Inserts a heading each time the Region changes.
 *
 * The list arrives already ordered by Region, then Favourite, then name, so a
 * change of regionCode is the group boundary — no sorting happens here.
 */
function withRegionHeadings(categories: Category[]) {
  const rows: {
    heading?: string;
    regionCode?: string;
    category?: Category;
  }[] = [];
  let current: string | null = null;
  for (const category of categories) {
    if (category.regionCode !== current) {
      current = category.regionCode;
      rows.push({ heading: category.regionLabel, regionCode: current });
    }
    rows.push({ category });
  }
  return rows;
}

function CategoryStar({
  providerId,
  kind,
  categoryId,
  active,
}: {
  providerId: number;
  kind: CatalogueKind;
  categoryId: number;
  active: boolean;
}) {
  const qc = useQueryClient();
  const toggle = useMutation({
    mutationFn: () => api.toggleCategoryFavourite(providerId, kind, categoryId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["categories"] }),
  });
  return (
    <span
      role="button"
      title={active ? "Unstar this category" : "Star this category"}
      onClick={(e) => {
        e.stopPropagation();
        toggle.mutate();
      }}
      className={cn(
        "shrink-0 rounded p-0.5 hover:bg-[var(--background)]",
        active ? "" : "opacity-0 group-hover/cat:opacity-100",
      )}
    >
      <Star
        className={cn(
          "size-3.5",
          active && "fill-current text-[var(--primary)]",
        )}
      />
    </span>
  );
}

function CategoryButton({
  active,
  onClick,
  name,
  count,
  star,
}: {
  active: boolean;
  onClick: () => void;
  name: string;
  count?: number;
  star?: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "group/cat flex w-full items-center gap-1.5 rounded-md px-2.5 py-1.5 text-left text-sm",
        active ? "bg-[var(--accent)] font-medium" : "hover:bg-[var(--accent)]",
      )}
    >
      {star}
      <span className="min-w-0 flex-1 truncate">{name}</span>
      {count != null ? (
        <span className="shrink-0 text-xs text-[var(--muted-foreground)] tabular-nums">
          {count}
        </span>
      ) : null}
    </button>
  );
}

function Channels({
  providerId,
  categoryId,
  onPlay,
}: {
  providerId: number;
  categoryId: number | null;
  onPlay: (r: PlayableRef) => void;
}) {
  const q = useQuery({
    queryKey: ["channels", providerId, categoryId],
    queryFn: () => api.channels(providerId, categoryId),
  });

  if (q.isPending) return <Notice title="Loading…" />;
  if (q.error) return <Notice title="Could not read the catalogue">{errorMessage(q.error)}</Notice>;
  if (!q.data.length)
    return (
      <Notice title="Nothing here yet">
        Sync this provider to pull its channels.
      </Notice>
    );

  return (
    <ul className="divide-y divide-[var(--border)]">
      {q.data.map((c) => {
        const ref: PlayableRef = {
          providerId,
          kind: "channel",
          refId: String(c.streamId),
        };
        return (
          <li
            key={c.streamId}
            className="group flex items-center gap-3 px-4 py-2 hover:bg-[var(--accent)]"
            onDoubleClick={() => onPlay(ref)}
          >
            <span className="w-10 shrink-0 text-right text-xs tabular-nums text-[var(--muted-foreground)]">
              {c.channelNumber ?? ""}
            </span>
            {c.icon ? (
              <img
                src={c.icon}
                alt=""
                loading="lazy"
                className="size-8 shrink-0 rounded object-contain"
                // Provider logo hosts die constantly; a broken icon should not
                // leave a broken-image glyph in every row.
                onError={(e) => {
                  e.currentTarget.style.visibility = "hidden";
                }}
              />
            ) : (
              <Tv className="size-8 shrink-0 p-1.5 text-[var(--muted-foreground)]" />
            )}
            <span className="min-w-0 flex-1 truncate text-sm">{c.name}</span>
            {c.isFavourite ? (
              <Star className="size-3.5 fill-current text-[var(--primary)]" />
            ) : null}
            <div className="flex shrink-0 items-center opacity-0 group-hover:opacity-100">
              <FavouriteToggle
                providerId={providerId}
                kind="channel"
                refId={ref.refId}
                active={c.isFavourite}
              />
              <Button size="sm" variant="ghost" onClick={() => onPlay(ref)}>
                <Play className="size-4" />
              </Button>
            </div>
          </li>
        );
      })}
    </ul>
  );
}

function Movies({
  providerId,
  categoryId,
  onPlay,
}: {
  providerId: number;
  categoryId: number | null;
  onPlay: (r: PlayableRef) => void;
}) {
  const q = useQuery({
    queryKey: ["movies", providerId, categoryId],
    queryFn: () => api.movies(providerId, categoryId),
  });

  if (q.isPending) return <Notice title="Loading…" />;
  if (q.error) return <Notice title="Could not read the catalogue">{errorMessage(q.error)}</Notice>;
  if (!q.data.length)
    return <Notice title="Nothing here yet">Sync this provider first.</Notice>;

  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-4 p-4">
      {q.data.map((m) => {
        const ref: PlayableRef = {
          providerId,
          kind: "movie",
          refId: String(m.streamId),
        };
        const progress =
          m.resume && m.resume.durationSecs
            ? (m.resume.positionSecs / m.resume.durationSecs) * 100
            : null;
        return (
          <button
            key={m.streamId}
            onClick={() => onPlay(ref)}
            className="group text-left"
          >
            <div className="relative aspect-[2/3] overflow-hidden rounded-lg bg-[var(--muted)]">
              {m.icon ? (
                <img
                  src={m.icon}
                  alt=""
                  loading="lazy"
                  className="size-full object-cover transition group-hover:scale-105"
                  onError={(e) => {
                    e.currentTarget.style.visibility = "hidden";
                  }}
                />
              ) : null}
              {progress != null ? (
                <div className="absolute inset-x-0 bottom-0 h-1 bg-black/50">
                  <div
                    className="h-full bg-[var(--primary)]"
                    style={{ width: `${progress}%` }}
                  />
                </div>
              ) : null}
            </div>
            <div className="mt-2 flex items-start gap-1">
              <p className="line-clamp-2 flex-1 text-xs">{m.name}</p>
              <FavouriteToggle
                providerId={providerId}
                kind="movie"
                refId={ref.refId}
                active={m.isFavourite}
              />
            </div>
          </button>
        );
      })}
    </div>
  );
}

function SeriesGrid({
  providerId,
  categoryId,
}: {
  providerId: number;
  categoryId: number | null;
}) {
  const q = useQuery({
    queryKey: ["series", providerId, categoryId],
    queryFn: () => api.series(providerId, categoryId),
  });

  if (q.isPending) return <Notice title="Loading…" />;
  if (q.error) return <Notice title="Could not read the catalogue">{errorMessage(q.error)}</Notice>;
  if (!q.data.length)
    return <Notice title="Nothing here yet">Sync this provider first.</Notice>;

  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-4 p-4">
      {q.data.map((s) => (
        <div key={s.seriesId} className="group">
          <Link
            to="/series/$seriesId"
            params={{ seriesId: String(s.seriesId) }}
            className="block text-left"
          >
            <div className="aspect-[2/3] overflow-hidden rounded-lg bg-[var(--muted)]">
              {s.cover ? (
                <img
                  src={s.cover}
                  alt=""
                  loading="lazy"
                  className="size-full object-cover transition group-hover:scale-105"
                  onError={(e) => {
                    e.currentTarget.style.visibility = "hidden";
                  }}
                />
              ) : null}
            </div>
          </Link>
          <div className="mt-2 flex items-start gap-1">
            <p className="line-clamp-2 flex-1 text-xs">{s.name}</p>
            <FavouriteToggle
              providerId={providerId}
              kind="series"
              refId={String(s.seriesId)}
            />
          </div>
        </div>
      ))}
    </div>
  );
}

/** One **Series**' **Episodes**, fetched on first open (see `commands::episodes`). */
export function SeriesDetail({
  providerId,
  seriesId,
  onPlay,
}: {
  providerId: number;
  seriesId: number;
  onPlay: (r: PlayableRef) => void;
}) {
  const q = useQuery({
    queryKey: ["episodes", providerId, seriesId],
    queryFn: () => api.episodes(providerId, seriesId),
  });

  if (q.isPending) return <Notice title="Fetching episodes…" />;
  if (q.error)
    return <Notice title="Could not fetch episodes">{errorMessage(q.error)}</Notice>;
  if (!q.data.length) return <Notice title="No episodes listed" />;

  const seasons = new Map<number, typeof q.data>();
  for (const e of q.data) {
    const list = seasons.get(e.season) ?? [];
    list.push(e);
    seasons.set(e.season, list);
  }

  return (
    <div className="overflow-y-auto p-6">
      {[...seasons.entries()].map(([season, episodes]) => (
        <section key={season} className="mb-8">
          <h3 className="mb-2 text-sm font-semibold">Season {season}</h3>
          <ul className="divide-y divide-[var(--border)]">
            {episodes.map((e) => (
              <li
                key={e.episodeId}
                className="group flex items-center gap-3 py-2"
              >
                <span className="w-8 shrink-0 text-right text-xs tabular-nums text-[var(--muted-foreground)]">
                  {e.episodeNumber}
                </span>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm">{e.title}</p>
                  {e.durationSecs ? (
                    <Muted className="text-xs">
                      {formatDuration(e.durationSecs)}
                    </Muted>
                  ) : null}
                </div>
                <Button
                  size="sm"
                  variant="ghost"
                  className="opacity-0 group-hover:opacity-100"
                  onClick={() =>
                    onPlay({
                      providerId,
                      kind: "episode",
                      refId: e.episodeId,
                    })
                  }
                >
                  <Play className="size-4" />
                </Button>
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}

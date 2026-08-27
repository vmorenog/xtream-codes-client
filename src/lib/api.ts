/**
 * The whole surface between the webview and Rust.
 *
 * The frontend has no network, filesystem or player access of its own — see
 * `src-tauri/capabilities/default.json`. Everything goes through these.
 *
 * Types mirror the `Serialize` impls in `src-tauri/src/db/model.rs`. Keep them
 * in step; there is no codegen holding them together.
 */
import { invoke } from "@tauri-apps/api/core";

export type CatalogueKind = "live" | "movie" | "series";
export type PlayableKind = "channel" | "movie" | "episode";

/** Wider than PlayableKind: a Series is favouritable but not playable. */
export type FavouriteKind = PlayableKind | "series" | "category";

export interface PlayableRef {
  providerId: number;
  kind: PlayableKind;
  refId: string;
}

export interface FavouriteRef {
  providerId: number;
  kind: FavouriteKind;
  refId: string;
}

export interface Entitlement {
  status: string | null;
  expiresAt: number | null;
  maxSessions: number | null;
}

export interface CatalogueCounts {
  channels: number;
  movies: number;
  series: number;
}

export interface Provider {
  id: number;
  name: string;
  baseUrl: string;
  username: string;
  lastSyncedAt: number | null;
  entitlement: Entitlement;
  counts: CatalogueCounts;
}

export interface Category {
  id: number;
  name: string;
  count: number;
  /** Always set; "OTHER" when the name gave nothing away. */
  regionCode: string;
  regionLabel: string;
  isFavourite: boolean;
}

export interface Region {
  code: string;
  label: string;
  visible: boolean;
  sortOrder: number;
  categoryCount: number;
  /** Arrived in a later Sync and was hidden on arrival. */
  isNew: boolean;
}

export interface SyncReport {
  channels: number;
  movies: number;
  series: number;
  /** Separator rows the Provider ships as Channels, dropped on the way in. */
  dividersDropped: number;
  newRegions: number;
}

export interface ResumePoint {
  positionSecs: number;
  durationSecs: number | null;
}

export interface Channel {
  streamId: number;
  name: string;
  icon: string | null;
  categoryId: number | null;
  channelNumber: number | null;
  hasArchive: boolean;
  epgChannelId: string | null;
  isFavourite: boolean;
}

export interface Movie {
  streamId: number;
  name: string;
  icon: string | null;
  categoryId: number | null;
  containerExtension: string | null;
  rating: number | null;
  addedAt: number | null;
  isFavourite: boolean;
  resume: ResumePoint | null;
}

export interface Series {
  seriesId: number;
  name: string;
  cover: string | null;
  plot: string | null;
  categoryId: number | null;
  rating: number | null;
}

export interface Episode {
  episodeId: string;
  seriesId: number;
  season: number;
  episodeNumber: number;
  title: string;
  plot: string | null;
  containerExtension: string | null;
  durationSecs: number | null;
  resume: ResumePoint | null;
}

export interface Programme {
  startTs: number;
  stopTs: number;
  title: string;
  description: string | null;
}

export interface SearchHit {
  kind: "channel" | "movie" | "series";
  refId: string;
  name: string;
}

/** One row of Continue Watching: In Progress, or a Series' Up Next. */
export interface ContinueItem {
  kind: PlayableKind;
  refId: string;
  name: string;
  icon: string | null;
  positionSecs: number | null;
  durationSecs: number | null;
  updatedAt: number;
  isUpNext: boolean;
  seriesId: number | null;
  seriesName: string | null;
  season: number | null;
  episodeNumber: number | null;
}

export interface Favourites {
  channels: Channel[];
  movies: Movie[];
  series: Series[];
}

export interface PlayerStatus {
  running: boolean;
  playing: boolean;
  paused: boolean;
  positionSecs: number | null;
  durationSecs: number | null;
  title: string | null;
}

export interface SyncProgress {
  providerId: number;
  stage: "categories" | "channels" | "movies" | "series" | "saving" | "done";
  items: number;
}

export const api = {
  mpvInstalled: () => invoke<boolean>("mpv_installed"),

  providers: () => invoke<Provider[]>("provider_list"),
  addProvider: (p: {
    name: string;
    baseUrl: string;
    username: string;
    password: string;
  }) => invoke<number>("provider_add", p),
  removeProvider: (providerId: number) =>
    invoke<void>("provider_remove", { providerId }),
  sync: (providerId: number) =>
    invoke<SyncReport>("provider_sync", { providerId }),

  regions: (providerId: number) => invoke<Region[]>("regions", { providerId }),
  setRegionVisible: (providerId: number, code: string, visible: boolean) =>
    invoke<void>("set_region_visible", { providerId, code, visible }),
  /** `codes` top first; anything omitted falls to the end. */
  setRegionOrder: (providerId: number, codes: string[]) =>
    invoke<void>("set_region_order", { providerId, codes }),
  setCategoryRegion: (
    providerId: number,
    kind: CatalogueKind,
    categoryId: number,
    code: string | null,
  ) =>
    invoke<void>("set_category_region", {
      providerId,
      kind,
      categoryId,
      code,
    }),
  toggleCategoryFavourite: (
    providerId: number,
    kind: CatalogueKind,
    categoryId: number,
  ) =>
    invoke<boolean>("toggle_category_favourite", {
      providerId,
      kind,
      categoryId,
    }),

  categories: (providerId: number, kind: CatalogueKind) =>
    invoke<Category[]>("categories", { providerId, kind }),
  channels: (providerId: number, categoryId: number | null) =>
    invoke<Channel[]>("channels", { providerId, categoryId }),
  movies: (providerId: number, categoryId: number | null) =>
    invoke<Movie[]>("movies", { providerId, categoryId }),
  series: (providerId: number, categoryId: number | null) =>
    invoke<Series[]>("series_list", { providerId, categoryId }),
  episodes: (providerId: number, seriesId: number, refresh = false) =>
    invoke<Episode[]>("episodes", { providerId, seriesId, refresh }),
  schedule: (providerId: number, streamId: number) =>
    invoke<Programme[]>("schedule", { providerId, streamId }),
  search: (providerId: number, query: string) =>
    invoke<SearchHit[]>("search", { providerId, query }),

  favourites: (providerId: number) =>
    invoke<Favourites>("favourites", { providerId }),
  continueWatching: (providerId: number, limit?: number) =>
    invoke<ContinueItem[]>("continue_watching", { providerId, limit }),

  toggleFavourite: (target: FavouriteRef) =>
    invoke<boolean>("toggle_favourite", { target }),
  isFavourite: (target: FavouriteRef) =>
    invoke<boolean>("is_favourite", { target }),
  saveWatchState: (
    playable: PlayableRef,
    positionSecs: number,
    durationSecs: number | null,
  ) =>
    invoke<void>("save_watch_state", { playable, positionSecs, durationSecs }),
  markWatched: (playable: PlayableRef) =>
    invoke<void>("mark_watched", { playable }),
  clearWatchState: (playable: PlayableRef) =>
    invoke<void>("clear_watch_state", { playable }),

  play: (playable: PlayableRef) => invoke<void>("play", { playable }),
  playerStatus: () => invoke<PlayerStatus>("player_status"),
  togglePause: () => invoke<void>("player_toggle_pause"),
  seek: (seconds: number) => invoke<void>("player_seek", { seconds }),
  stop: () => invoke<void>("player_stop"),
};

/** Rust errors arrive as plain strings; anything else is a bug in the bridge. */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Something went wrong.";
}

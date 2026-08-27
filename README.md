# xtream-codes-client

A personal macOS client for watching IPTV served over the Xtream Codes API —
live TV, movies and series, with no ads and nothing between you and the picture.

> **Status:** stack decided, not yet scaffolded. See [`docs/adr/`](docs/adr/).

## Stack

| Layer | Choice | Why |
|---|---|---|
| Shell | Tauri v2 (Rust) | Sidecar management, no CORS, keychain, 10MB binary — [ADR-0001](docs/adr/0001-tauri-over-electron.md) |
| Playback | mpv as an IPC-driven sidecar | Plays raw MPEG-TS natively, hardware-decoded — [ADR-0002](docs/adr/0002-mpv-sidecar-for-playback.md) |
| Frontend | React 19 + TypeScript + Vite | TanStack Router for typed routes, TanStack Query over `invoke()` |
| Styling | Tailwind v4 + shadcn/ui | Grids, dialogs and a command palette, already built |
| Storage | SQLite in Rust (FTS5) | 80k-row catalogue, instant search — [ADR-0004](docs/adr/0004-sqlite-mirror-with-manual-sync.md) |
| Secrets | macOS Keychain | Provider passwords never touch the database |

## Vocabulary

The words this project uses are fixed in [`CONTEXT.md`](CONTEXT.md). A
**Provider** is a subscription, a **Channel** is live TV, a **Playable** is
anything you can hit play on. Read it before writing code — it is short.

## What it does

- Holds several **Providers**; every catalogue row is scoped to one
- Browses **Channels**, **Movies** and **Series** by **Category**
- Shows the **Schedule** (EPG) for a **Channel**
- Plays anything via mpv, with **Favourites** and **Resume Points**
- **Never syncs on launch.** Opens instantly off the local mirror; you sync when
  you choose to. This is deliberate — see
  [ADR-0004](docs/adr/0004-sqlite-mirror-with-manual-sync.md)

## Requirements

- macOS (Apple Silicon or Intel)
- `brew install mpv` — the app detects it and will not play without it
  ([ADR-0003](docs/adr/0003-mpv-is-a-system-dependency.md))
- Rust toolchain + Node for building

## Getting started

Not scaffolded yet. This section gets install/run/test commands once
`npm create tauri-app` has been run.

## Credentials

**Provider** credentials are secrets and live in the macOS Keychain, never in
the repo and never in the database. See [`.env.example`](.env.example) for the
shape used in development only.

Xtream **Stream URLs** embed the username and password in the path
(`/live/<user>/<pass>/<id>.ts`). A logged stream URL is a leaked password.

## Legal

This is a *client*. It ships no content and no credentials. You are responsible
for holding a legitimate subscription to anything you point it at.

## License

[MIT](LICENSE)

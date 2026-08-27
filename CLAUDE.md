# xtream-codes-client

IPTV client that talks to Xtream Codes providers (live TV, VOD, series).

## Stack

Tauri v2 (Rust) shell + React 19 / TypeScript / Vite webview, TanStack Router
and Query, Tailwind v4 + shadcn/ui, SQLite via rusqlite, playback delegated to
an mpv sidecar over its JSON IPC socket.

The decisions and the rejected alternatives are in [`docs/adr/`](docs/adr/).
Read ADR-0002 and ADR-0004 before touching playback or sync — both look wrong
until you know why.

Nothing is scaffolded yet. Do not add a dependency that carries lock-in
(database, state library, player) without asking first; when one is added,
record it as an ADR.

## Hard rules

- **Never commit credentials.** **Provider** `username`/`password` are secrets.
  They live in the macOS Keychain at runtime; SQLite stores only the Provider's
  name and base URL. `.env.example` holds the shape for dev, never real values.
- **Never log a full stream URL.** Xtream Codes puts the username and
  password in the path (`/live/<user>/<pass>/<id>.ts`), so a logged URL is a
  leaked credential. Redact before logging.
- **No content ships here.** No playlists, no recordings, no provider hosts.

## Domain language

**Use the terms in [`CONTEXT.md`](CONTEXT.md) exactly.** A subscription is a
**Provider**, never an "account". A live service is a **Channel**, never a
"stream" — "Stream URL" means the credential-bearing address only. Anything
playable is a **Playable**.

Xtream Codes exposes a small JSON API plus direct stream paths — see
[`docs/xtream-api.md`](docs/xtream-api.md) for the endpoint reference.

## Behaviours that look like bugs but are not

- **The app never syncs on launch.** Manual only, plus a staleness notice.
  ADR-0004.
- **mpv is not bundled.** It is a Homebrew dependency, detected at startup.
  ADR-0003.
- **Video does not render in the webview.** It is a separate mpv process.
  ADR-0002.

## Conventions

- Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`).
- Branch off `main`; PRs, no direct pushes once work starts.

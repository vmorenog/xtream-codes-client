# xtream-codes-client

IPTV client that talks to Xtream Codes providers (live TV, VOD, series).

## Stack

Tauri v2 (Rust) shell + React 19 / TypeScript / Vite webview, TanStack Router
and Query, Tailwind v4 + shadcn/ui, SQLite via rusqlite, playback delegated to
an mpv sidecar over its JSON IPC socket.

The decisions and the rejected alternatives are in [`docs/adr/`](docs/adr/).
Read ADR-0002 and ADR-0004 before touching playback or sync — both look wrong
until you know why.

Do not add a dependency that carries lock-in (database, state library, player)
without asking first; when one is added, record it as an ADR.

## Where things live

```
src/lib/api.ts          the ONLY bridge to Rust. Types are hand-kept in step
                        with the Serialize impls in src-tauri/src/db/model.rs —
                        change one, change the other.
src-tauri/src/xtream/   HTTP client; de.rs holds the lenient deserializers
src-tauri/src/db/       SQLite mirror. schema.rs migrations are append-only.
src-tauri/src/player/   mpv sidecar over a Unix socket
src-tauri/src/commands.rs   every capability the webview has
```

The webview has no network, filesystem or process permissions — see
`src-tauri/capabilities/default.json`. Adding a frontend capability means
adding a Rust command, not widening that file.

## Checks

```sh
pnpm typecheck && pnpm build
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test --lib
```

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
- **`*.ts` is not in `.gitignore`.** MPEG transport streams share the extension
  with TypeScript; ignoring it would hide `vite.config.ts`. Captured media goes
  in `media/`.
- **A Channel cannot have a Resume Point.** The SQL CHECK constraint enforces
  it. Live has no beginning to return to.
- **Up Next is the lowest Unwatched Episode, not the one after the last
  watched.** Recency looks more intuitive and is wrong: it sends you backwards
  after a rewatch. ADR-0006 has the three cases. Do not "fix" this.
- **Finished Playables keep their row.** Completion is recorded, not deleted —
  deleting it is what made Up Next uncomputable. ADR-0006.
- **Sync deletes Favourites.** Deliberately: it drops rows whose Playable has
  vanished or been renumbered into a different one. ADR-0007.
- **Region is never parsed from a Channel name.** Only 29% carry a prefix and
  the codes contradict the Categories (`|GB|` vs `UK -`). It comes from the
  Category. ADR-0008.
- **Channel counts do not match the Provider's.** Divider rows
  (`======= BULGARIAN =======`) are dropped at Sync. ADR-0008.
- **"Pin", "channel group" and "language" are banned words.** They are
  **Favourite**, **Category** and **Region**. See CONTEXT.md.

## Conventions

- Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`).
- Branch off `main`; PRs, no direct pushes once work starts.

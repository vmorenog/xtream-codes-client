# xtream-codes-client

IPTV client that talks to Xtream Codes providers (live TV, VOD, series).

## State of the repo

Bootstrap only. **No tech stack has been decided.** Do not introduce a
language, framework, package manager or player library without asking first.
When a stack decision is made, record it as an ADR in `docs/decisions/`.

## Hard rules

- **Never commit credentials.** Provider `username`/`password` and the base
  URL are secrets. They live in `.env` (git-ignored) or platform secure
  storage. `.env.example` holds the shape only, never real values.
- **Never log a full stream URL.** Xtream Codes puts the username and
  password in the path (`/live/<user>/<pass>/<id>.ts`), so a logged URL is a
  leaked credential. Redact before logging.
- **No content ships here.** No playlists, no recordings, no provider hosts.

## Domain notes

Xtream Codes exposes a small JSON API plus direct stream paths — see
[`docs/xtream-api.md`](docs/xtream-api.md) for the endpoint reference.

## Conventions

- Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`).
- Branch off `main`; PRs, no direct pushes once work starts.

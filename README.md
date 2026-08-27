# xtream-codes-client

An IPTV client for watching live TV, movies and series served over the
[Xtream Codes](https://en.wikipedia.org/wiki/IPTV) API.

> **Status:** repo bootstrap. No tech stack has been chosen yet — see
> [`docs/decisions/`](docs/decisions/) once the first ADR lands.

## What it will do

- Connect to an Xtream Codes provider with `server URL + username + password`
- Browse live channels, VOD and series, with categories and EPG
- Play streams (HLS / TS) with the usual transport controls
- Remember favourites and playback position

## Getting started

Nothing to run yet. Once a stack is picked, this section documents install,
run and test commands.

## Credentials

Provider credentials are secrets. They never go in the repo — use a local
`.env` (git-ignored) or the platform's secure storage. See
[`.env.example`](.env.example).

## Legal

This project is a *client*. It ships no content and no provider credentials.
Users are responsible for holding a legitimate subscription to any service
they connect it to.

## License

[MIT](LICENSE)

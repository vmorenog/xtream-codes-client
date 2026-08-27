# Contributing

## Workflow

1. Branch off `main`.
2. Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`).
3. Open a PR; keep it scoped to one change.

## Decisions

Anything that constrains the project long-term (language, framework, player,
storage, distribution target) gets an ADR in `docs/adr/`, copied from
`0000-template.md` and numbered in sequence.

## Secrets

Never commit a `.env`, a provider URL, or a stream URL — Xtream Codes stream
paths contain the username and password.

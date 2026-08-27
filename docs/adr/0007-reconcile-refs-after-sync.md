# Favourites and Watch State are reconciled against a name snapshot after Sync

**Favourite** and `watch_state` rows reference a **Playable** by
`(provider_id, kind, ref_id)` with no foreign key into the **Catalogue**, and
`replace_catalogue` rewrites the Catalogue wholesale precisely because
**Providers** renumber ids between panel upgrades (ADR-0004). Those two facts
together mean a Favourite on `stream_id 42` silently becomes a Favourite on
whatever is 42 *after* the next **Sync** — not a broken row, a wrong one.

So each row stores the Playable's name as a snapshot, and every Sync drops rows
whose `ref_id` has disappeared, or whose name no longer matches what that id now
resolves to.

## Considered options

- **Purge missing ids only.** Rejected: a renumber does not remove id 42, it
  reassigns it, so the dangerous half — silent aliasing — survives untouched.
- **Key on name instead of id.** Rejected: names are not unique on a real
  Provider (duplicate `LA 1` across categories is routine), and a retitle breaks
  it just as badly. Trades a rare failure for a common one.
- **Leave it.** Rejected: the failure is invisible. A Favourite that plays the
  wrong **Channel** reads as our bug, and **Continue Watching** could resume a
  film at another film's timestamp.

## Consequences

- A Provider that merely retitles something (`LA 1` to `LA 1 HD`) loses the
  Favourite. Accepted: losing a star beats pointing it at the wrong Channel.
- Sync gains a reconciliation step, so it can now delete **Viewer** data. It
  runs inside the same transaction as the Catalogue replace.

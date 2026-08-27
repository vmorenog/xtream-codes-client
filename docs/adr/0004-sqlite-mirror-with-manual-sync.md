# The Catalogue is a local SQLite mirror, synced manually

A **Provider**'s `get_live_streams` response alone can be 20-50MB, and a full
**Catalogue** runs to 80k **Channels** plus tens of thousands of **Movies** and
**Episodes**. That is mirrored into SQLite in the Rust layer: FTS5 gives instant
name search, **Schedule** lookups become indexed time-range queries, and
**Favourites** and **Resume Points** get a real schema. **Sync** happens only
when the **Viewer** asks for it, or is nudged when the mirror is stale.

## Considered options

- **JSON files on disk.** Rejected: 50MB parsed into JS memory every launch, and
  search becomes a hand-written linear scan.
- **IndexedDB in the webview.** Rejected: WKWebView can evict it with the cache,
  and it would mean pushing 50MB across the IPC bridge to write.
- **Sync on launch, blocking or background.** Rejected deliberately — see below.

## Consequences

- **The app never syncs on its own. This is intentional, not an oversight.**
  Launch reads straight from SQLite and is instant; a stale **Catalogue** is a
  quiet notice, never a spinner and never a list that reorders while you browse.
  A **Channel** added by the **Provider** yesterday stays invisible until the
  next **Sync**, and that is the accepted cost.
- Schema migrations are now a thing this project owns.

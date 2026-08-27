# A Channel's Region comes from its Category, and Dividers are dropped at Sync

A **Provider** catalogue is not clean data. Ours holds 15,353 **Channels** in
290 live **Categories**, and the obvious way to group them by language — parsing
the `|ES|` prefix off the Channel name — does not survive contact with it.

## Region is derived from the Category

Measured on a real catalogue:

- Only **4,420 of 15,353 Channels (29%)** carry a `|XX|` prefix at all.
- The codes are not two letters: `|ARG|`, `|RSA|` are in there too.
- Channel and Category codes **disagree** — Channels say `|GB|`, Categories say
  `UK - `.
- **179 of 290 Categories** carry a `XX - ` prefix. The other 111 are bare
  country names: `HOLLAND`, `INDIA`, `BRAZIL`, `CZECH`, `EX-YU`.

So the **Region** is resolved per Category at **Sync** time, in two passes: a
code table for the prefixed ones (with `UK` and `GB` both mapping to the United
Kingdom), then a name lookup for the rest — bare countries (`HOLLAND`), word
prefixes (`LAT -`, `Polska -`) and language groupings (`ENGLISH -`, `ESPAÑA -`).
A Channel's Region is simply its Category's; Channels are never parsed.

Measured against the same real catalogue, this tags **309 of 319 Categories
(97%)**. The ten left over are correct to leave untagged — `ADULTS`, `MLB`,
`NBA`, `NFL`, `PEACOCK`, `PPV - SPORT`, `SPFL Championship`, `SPFL PREMIERSHIP`,
`MLS`, `No Category` — none of them is a place. They land in **Other**, a real
Region bucket rather than a null, so every Category has exactly one Region to be
filtered and ordered by.

The term is **Region**, not language: the same field holds `ES` (a country),
`ARABIC` (a language) and `EX-YU` (a defunct state). Providers conflate them and
the glossary says so rather than pretending otherwise.

## Dividers are dropped

Providers ship rows like `======= BULGARIAN =======` and `***** SWISS *****` as
**Channels**, purely so dumber players draw a line. They are unplayable. They
are detected by name shape and dropped during Sync, so they never reach the
mirror, a list or a search result.

## Hidden Regions still live in the mirror

Hiding a Region filters rails and search; it does not skip the Sync. Toggling a
Region back on is then instant with no refetch, and a Region cannot be tagged
before it has been downloaded at least once.

## Consequences

- **Channel counts will not match what the Provider advertises**, because
  Dividers are gone. That is deliberate.
- The heuristic will mis-tag a handful of Categories. The manual override is not
  a nicety, it is the release valve that makes the heuristic acceptable.
- **Once the Viewer hides any Region, Regions new in a later Sync arrive
  hidden**, and Sync reports how many. Curation survives a Sync; nothing
  vanishes without a count saying it happened. Before any curation, everything
  shows.
- The code table is data we now maintain. It will need entries as Providers
  invent codes. Two of its entries exist purely because a Provider misspells a
  country (`COLUMBIA`, `HUNGARIA`); recognising their spelling is the job, not
  correcting it.
- Upgrading an existing database backfills Regions on open and rebuilds the
  search index, so it works before the next Sync rather than after it.

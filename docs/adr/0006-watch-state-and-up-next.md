# Watch State is stored, and Up Next is the lowest Unwatched Episode

`resume_points` is replaced by a single `watch_state` table holding one row per
**Playable**: a state (`in_progress` | `watched`), a nullable position, and
`updated_at`. Completion is now *recorded* rather than deleted, because the old
behaviour — dropping the row past 95% — destroyed the only evidence that an
**Episode** had been finished, which made **Up Next** impossible to compute.

One table rather than two: a `resume_points` row and a `watched` row for the
same Playable would be a contradiction nothing prevents, and every **Continue
Watching** query would become a union with dedup rules to keep straight.

## Up Next is the lowest Unwatched Episode

Deliberately *not* "the Episode after the last one you watched". Three cases
decided it, and they are written out as dialogue in `CONTEXT.md`:

- **Rewatch.** Watch E1–E5, then rewatch E2. Recency would offer E3, already
  seen. Lowest-Unwatched correctly offers E6.
- **Misclick.** Land on S3E1 of an unseen show. Highest-Watched would make S3E2
  permanent and strand S1E1. Lowest-Unwatched still offers S1E1, so one bad
  click cannot poison a **Series**.
- **Deliberate skip.** The one case this rule loses: a skipped Episode stays
  **Unwatched** and keeps being offered. The escape hatch is marking it
  **Watched** by hand.

If someone later "fixes" this to use recency, they are reintroducing the
rewatch bug. That is what this ADR exists to prevent.

## Consequences

- A **Channel** still holds no **Watch State**; live has no end to reach. The
  CHECK constraint keeps enforcing it.
- Rows now accumulate instead of being cleaned up on completion. That is the
  point, and the volume is bounded by what the **Viewer** actually watches.

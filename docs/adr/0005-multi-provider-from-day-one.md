# Multi-Provider schema from day one

Every **Catalogue** table carries a `provider_id` from the first migration, even
though only one **Provider** will be configured at first. IPTV subscriptions
expire and get replaced often enough that a second one is a matter of when, not
if — and retrofitting the column later means migrating the entire cached mirror
and rewriting every query. The cost of carrying it now is one column and one
join; the cost of adding it later is a rewrite.

## Consequences

- **Favourites** and **Resume Points** are scoped to a **Provider**, so replacing
  a subscription does not silently repoint them at a different **Channel** that
  happens to share an id.

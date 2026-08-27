# Xtream Codes Client

A personal desktop client for watching IPTV served over the Xtream Codes API.
This document is a glossary, not a spec — it fixes what the words mean so the
code and the conversation agree.

## Language

### Access

**Provider**:
A single IPTV subscription, identified by a base URL, a username and a password.
_Avoid_: account, service, portal, panel, source

**Viewer**:
The person using the app. There is exactly one, and they are never
authenticated — the app has no login of its own.
_Avoid_: user, account

**Entitlement**:
What a **Provider** says the **Viewer** is allowed to do right now — active or
expired, expiry date, and the cap on simultaneous streams.
_Avoid_: subscription status, user_info

### Catalogue

**Catalogue**:
Everything a single **Provider** offers, as mirrored locally. Composed of
**Channels**, **Movies** and **Series**.
_Avoid_: library, playlist, lineup

**Channel**:
One live television service, continuously broadcasting. Belongs to a
**Category**.
_Avoid_: stream, station, feed

**Movie**:
One on-demand film, watchable start to finish at any time.
_Avoid_: VOD, video, title

**Series**:
An on-demand show, containing **Seasons**, which contain **Episodes**.
_Avoid_: show, TV series

**Category**:
A **Provider**-defined grouping such as `ES - FUTBOL`. Categories never cross
the Channel / Movie / Series boundary — each kind has its own set. The Viewer
cannot create or edit one.
_Avoid_: genre, group, channel group, folder

**Region**:
The audience a **Category** is aimed at. A country or a language, because
Providers conflate the two in one field — `ES - FUTBOL` is a country,
`ARABIC - BEIN SPORTS` is a language. Derived from the Category, never from a
**Channel** name, and correctable by the **Viewer**.
_Avoid_: language, country, locale, market

**Divider**:
A row a **Provider** ships in place of a **Channel** purely to draw a line —
`======= BULGARIAN =======`. Never playable, and not part of the **Catalogue**.
_Avoid_: separator, header, placeholder

**Playable**:
Any single thing that can be played: a **Channel**, a **Movie**, or an
**Episode**. The term exists because favourites, resume and playback treat all
three identically.
_Avoid_: item, media, content

**Stream URL**:
The credential-bearing URL that a **Playable** resolves to at playback time.
Contains the **Provider** username and password in its path, and is therefore a
secret.
_Avoid_: link, source URL

### Schedule

**Programme**:
One scheduled broadcast on one **Channel**, with a title, description, start
and end.
_Avoid_: show, event, EPG entry

**Schedule**:
The collection of **Programmes** for a **Channel** over a time window. What
other apps call the EPG.
_Avoid_: EPG, guide, listings

### Viewing

**Sync**:
The act of refreshing the local **Catalogue** from a **Provider**. Explicit and
resumable, never silent on launch.
_Avoid_: refresh, update, import, fetch

**Favourite**:
Something the **Viewer** has singled out for quick access: a **Channel**,
**Movie**, **Episode**, **Series** or **Category**. Wider than **Playable**,
because a Series is worth keeping even though you play its Episodes, and a
Category is worth keeping even though you play nothing at all.
_Avoid_: bookmark, starred, pinned, pin

**Resume Point**:
How far into a **Movie** or **Episode** the **Viewer** got. **Channels** have
no Resume Point — live has no beginning to return to.
_Avoid_: progress, watch position, bookmark

**Watched**:
A **Movie** or **Episode** the **Viewer** finished. A **Channel** is never
Watched — live has no end to reach.
_Avoid_: seen, completed, played

**Watch State**:
Which of three a **Playable** is in: **Unwatched** (no record), **In Progress**
(has a **Resume Point**), or **Watched**.
_Avoid_: status, progress

**Up Next**:
The lowest-numbered **Unwatched Episode** of a **Series** the **Viewer** has
started. Ordering is by season then episode number, never by when the Viewer
last watched.
_Avoid_: next up, continue, resume

**Continue Watching**:
The collection the **Viewer** is offered to pick up: every **In Progress**
**Playable**, plus the **Up Next** of every started, unfinished **Series**. At
most one entry per Series — an In Progress **Episode** stands in for that
Series' Up Next rather than appearing alongside it.
_Avoid_: recent, history, watchlist

**Session**:
One instance of a **Playable** being played. Bounded by the **Provider**'s
simultaneous-stream cap from the **Entitlement**.
_Avoid_: connection, playback

## Relationships

- A **Viewer** holds one or more **Providers**
- A **Favourite** points at a **Channel**, **Movie**, **Episode**, **Series** or **Category**
- A **Provider** grants one **Entitlement** and offers one **Catalogue**
- A **Catalogue** contains many **Channels**, **Movies** and **Series**
- A **Category** belongs to exactly one **Region**, or to none when it cannot be told
- A **Region** is shown or hidden, and the **Viewer** decides the order of the shown ones
- A **Series** contains **Seasons**, which contain **Episodes**
- A **Channel**, a **Movie** and an **Episode** are each a **Playable**
- A **Playable** resolves to exactly one **Stream URL**
- A **Channel** has a **Schedule** of many **Programmes**
- A **Movie** or **Episode** has at most one **Resume Point**; a **Channel** has none
- A **Movie** or **Episode** holds one **Watch State**; a **Channel** holds none
- A started **Series** has exactly one **Up Next**, or none once every **Episode** is **Watched**
- **Continue Watching** draws from **Resume Points** and **Up Next**, never from **Favourites**
- A **Provider** caps how many **Sessions** may run at once

## Example dialogue

> **Dev:** "When the **Viewer** favourites something, do I store the **Stream URL**?"
> **Domain expert:** "Never. The **Stream URL** carries the **Provider**'s
> password, and it changes if the subscription is replaced. Store the
> **Playable**'s id and resolve the URL fresh at playback."
>
> **Dev:** "And if they favourite a **Channel** and then the **Provider** expires?"
> **Domain expert:** "The **Favourite** survives — it points at the **Playable**,
> which belongs to a **Catalogue**, which belongs to that **Provider**. It just
> stops being playable until the **Entitlement** is active again."
>
> **Dev:** "Does a **Channel** get a **Resume Point** if I pause it?"
> **Domain expert:** "No. Pausing a live **Channel** pauses the **Session**, but
> there is nothing to resume to later — the broadcast moved on."
>
> **Dev:** "The **Viewer** watched S1E1 to E5, then rewatched E2. What is
> **Up Next**?"
> **Domain expert:** "E6. **Up Next** is the lowest **Unwatched** **Episode**,
> not the one after whatever they touched last — otherwise a rewatch would send
> them backwards through episodes they have already seen."
>
> **Dev:** "And if they misclick into S3E1 of something they have never seen?"
> **Domain expert:** "**Up Next** is still S1E1. That is the whole point of
> using the lowest **Unwatched** rather than the highest **Watched** — one bad
> click must not strand the rest of the **Series**."
>
> **Dev:** "What if they deliberately skip an **Episode**?"
> **Domain expert:** "Then it stays **Unwatched** and **Up Next** keeps offering
> it. Marking it **Watched** by hand is how the **Viewer** says 'I meant to skip
> that'."
>
> **Dev:** "Can I work out a **Channel**'s **Region** from its name? They mostly
> start with `|ES|`."
> **Domain expert:** "No. Fewer than a third of Channels carry one, and the
> codes disagree with the **Categories** — Channels say `|GB|` where Categories
> say `UK`. A Channel's Region is whatever its Category's Region is."
>
> **Dev:** "What Region is `HOLLAND`? There is no prefix."
> **Domain expert:** "The Netherlands. Roughly a third of Categories are bare
> country names like that, so the name itself has to be looked up when there is
> no prefix. Anything still unrecognised has no Region, and the **Viewer** can
> set it by hand."

## Flagged ambiguities

- **"account"** was used for both the IPTV subscription and the person watching.
  Resolved: the subscription is a **Provider**, the person is the **Viewer**.
  The app has no accounts of its own.
- **"stream"** was used for both a live **Channel** and the credential-bearing
  **Stream URL**. Resolved: **Channel** is the thing you watch, **Stream URL**
  is the secret address it resolves to. "Stream" alone is banned.
- **Category** could have been one shared set across all kinds. Resolved: the
  Xtream API returns separate category sets for live, VOD and series, and they
  are not interchangeable.
- **"finished"** was used for both a **Playable** reaching its end and a
  **Series** having no **Episodes** left. Resolved: a Playable is **Watched**; a
  **Series** with no **Up Next** is finished, which is derived, not stored.
- **"continue watching"** was used loosely for both the collection and the act.
  Resolved: **Continue Watching** is the collection. The act is just playing.
- **Favourite** was first defined as a **Playable**, which excluded **Series**.
  Resolved: a Favourite is wider than a Playable. You keep a show; you play an
  **Episode** of it. It was later widened again to include **Category**.
- **"pin"** was used for singling out a **Category**, which is the same act as
  starring a **Channel**. Resolved: there is one concept, **Favourite**. "Pin"
  is banned so the two do not drift apart in the code.
- **"channel group"** was used for what the Provider calls a **Category**.
  Resolved: **Category**. There is no Viewer-created grouping in this app.
- **"language"** was used for a setting that filters on things like `EX-YU` and
  `ARABIC`, which are not languages, and `ES`, which is not one either.
  Resolved: **Region**, defined as country-or-language, because the Provider
  does not distinguish them.

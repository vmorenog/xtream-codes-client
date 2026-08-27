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
A **Provider**-defined grouping. Categories never cross the Channel / Movie /
Series boundary — each kind has its own set.
_Avoid_: genre, group, folder

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
A **Playable** the **Viewer** has pinned for quick access.
_Avoid_: bookmark, starred, pinned

**Resume Point**:
How far into a **Movie** or **Episode** the **Viewer** got. **Channels** have
no Resume Point — live has no beginning to return to.
_Avoid_: progress, watch position, bookmark

**Session**:
One instance of a **Playable** being played. Bounded by the **Provider**'s
simultaneous-stream cap from the **Entitlement**.
_Avoid_: connection, playback

## Relationships

- A **Viewer** holds one or more **Providers**
- A **Provider** grants one **Entitlement** and offers one **Catalogue**
- A **Catalogue** contains many **Channels**, **Movies** and **Series**
- A **Series** contains **Seasons**, which contain **Episodes**
- A **Channel**, a **Movie** and an **Episode** are each a **Playable**
- A **Playable** resolves to exactly one **Stream URL**
- A **Channel** has a **Schedule** of many **Programmes**
- A **Movie** or **Episode** has at most one **Resume Point**; a **Channel** has none
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

# Xtream Codes API reference

All calls hit `{base_url}` (usually `http://host:port`) and authenticate with
the same `username` / `password` query pair on every request.

## Auth / handshake

```
GET {base}/player_api.php?username={u}&password={p}
```

Returns `user_info` (status, expiry, max connections, allowed output formats)
and `server_info` (url, port, https port, timezone). Use it as the login
check: `user_info.auth == 1` and `status == "Active"`.

## Catalogue

`GET {base}/player_api.php?username={u}&password={p}&action={action}`

| action | returns |
|---|---|
| `get_live_categories` | live TV categories |
| `get_live_streams` | live channels (`&category_id=` to filter) |
| `get_vod_categories` | movie categories |
| `get_vod_streams` | movies |
| `get_vod_info` | one movie, `&vod_id=` |
| `get_series_categories` | series categories |
| `get_series` | series list |
| `get_series_info` | seasons + episodes, `&series_id=` |
| `get_short_epg` | next N programmes, `&stream_id=&limit=` |
| `get_simple_data_table` | full EPG for a stream, `&stream_id=` |

EPG text fields come back base64-encoded.

## Streams

```
Live    {base}/live/{u}/{p}/{stream_id}.{ts|m3u8}
Movie   {base}/movie/{u}/{p}/{stream_id}.{container_extension}
Series  {base}/series/{u}/{p}/{episode_id}.{container_extension}
XMLTV   {base}/xmltv.php?username={u}&password={p}
```

> These URLs embed the credentials. Treat them as secrets: never log them
> whole, never put them in error reports or analytics.

## Gotchas

- Providers cap concurrent connections (`max_connections`); exceeding it
  fails the stream, not the API call.
- Field types are inconsistent across providers — ids and flags arrive as
  strings or numbers depending on the panel. Coerce on parse.
- Many panels are plain HTTP. Prefer the `https_port` from `server_info`
  when it is present.

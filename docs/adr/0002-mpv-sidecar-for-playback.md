# Playback runs in an mpv sidecar, not in the webview

Xtream providers serve live **Channels** as raw MPEG-TS, which browsers cannot
play natively — an in-webview player would mean mpegts.js remuxing to fMP4 over
MSE, making stutter, audio desync and stream-drop recovery our problem. mpv
handles TS, HLS and every VOD container natively with hardware decoding, so we
run it as a separate process and drive it over its JSON IPC socket (load, pause,
seek, track selection, position reporting).

## Considered options

- **mpv embedded in the app window** (`--wid` / libmpv render callback). Same
  playback quality with one seamless window, but requires native view-handle
  glue in Rust/ObjC. Deferred, not rejected — the IPC protocol is identical, so
  embedding later is a rendering change, not an architecture change.
- **hls.js / mpegts.js in the webview.** Rejected: raw TS over MSE is the
  fragile path, and stream reliability is the entire point of this project.
- **Hand off to IINA/VLC.** Rejected: no **Resume Point**, no in-app control,
  a launcher rather than a client.

## Consequences

- Two windows until embedding is done: the catalogue window and mpv's own.
- **Stream URLs** are passed to mpv over the IPC socket. They contain the
  **Provider** password, so mpv's command line and our logs must never carry
  them in the clear.

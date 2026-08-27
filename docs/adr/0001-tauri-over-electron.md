# Tauri v2 over Electron

The client needs to make HTTP calls to Xtream panels that send no CORS headers,
spawn and supervise an external player process, and hold credentials in the OS
keychain — all of which are backend concerns, not webview concerns. Electron's
main advantage is Chromium's consistent codec support, but ADR-0002 moves video
out of the webview entirely, so that advantage does not apply here. Tauri gives
first-class sidecar management and a ~10MB binary instead of ~200MB of Chromium
that would never decode a frame.

## Consequences

- The API client, SQLite access and mpv control all live in Rust, not TypeScript.
- WKWebView is the rendering target, so Chromium-only CSS and JS APIs are off
  the table.

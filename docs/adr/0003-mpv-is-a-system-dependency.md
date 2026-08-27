# mpv is a system dependency, not a bundled sidecar binary

The app detects mpv on `PATH` and in `/opt/homebrew/bin` at startup and shows a
`brew install mpv` setup screen if it is missing, rather than shipping mpv
inside the `.app`. Bundling would add ~40MB, force us to handle the arm64/x86
split, and require the nested binary to be separately signed and notarized or
Gatekeeper refuses to launch it. For a personal single-machine app that cost
buys nothing, and Homebrew keeps mpv patched for free.

## Consequences

- The app is not distributable to anyone without Homebrew. If that ever changes,
  this decision is the one to revisit — the sidecar path stays open because
  ADR-0002's IPC protocol does not care where the binary came from.
- mpv can update underneath us and change playback behaviour. Pinning is the
  user's job, not the app's.

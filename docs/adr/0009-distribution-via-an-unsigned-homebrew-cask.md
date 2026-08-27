# Distributed as an unsigned Homebrew cask, macOS only

The app is shared through a cask in our own tap rather than a `.dmg` people
download from a web page:

```sh
brew install --cask --no-quarantine vmorenog/tap/xtream
```

The tap exists mainly because of ADR-0003. mpv is a system dependency, not a
bundled binary, so any download-a-file route means telling every recipient to
run `brew install mpv` first. A cask declares `depends_on formula: "mpv"` and
the problem disappears — one command installs both, in the right order.

## Not signed, and not notarized

The Apple Developer Program is $99/year, and the audience is a handful of
people who already use Homebrew. Unsigned means Gatekeeper quarantines the app,
which is why the install line carries `--no-quarantine`: Homebrew Cask *applies*
quarantine by default and a cask cannot waive it on the user's behalf.

Bundling mpv was rejected for the same reason signing was skipped, and it is
worth being explicit because the reasoning inverts once you are unsigned: a
nested unsigned binary inside an unsigned bundle is the fiddliest Gatekeeper
case there is. Bundling would have added risk to remove a step this audience
can already perform.

## Deliberately macOS only, for now

Tauri supports Windows and Linux, and the port is small and well understood.
Recorded here so the next person does not have to rediscover it:

- `player/mpv.rs` uses `UnixStream`; Windows needs named pipes. This is the only
  real work.
- mpv discovery paths are Homebrew's, and the fallback shells out to
  `/usr/bin/which`.
- `--hwdec=videotoolbox` is Apple-only; `auto` elsewhere.
- `keyring` is built with `apple-native`; Windows and Linux need their own
  features.
- `titleBarStyle: "Overlay"` is a macOS-only Tauri option.
- CI runs `macos-latest` only, and would need a matrix.

The blocker is not the code, it is that we would own three platforms we cannot
test on, and Windows brings its own code-signing bill.

## Consequences

- **The repository is public.** A tap cannot fetch a release asset from a
  private repo. The project ships no content and no credentials, but a
  **Provider**'s hostname must never reach an issue, a log or a screenshot.
- Releases are universal binaries, so one URL and one checksum serve both Apple
  Silicon and Intel. Roughly five extra CI minutes and ~10MB; only the Rust
  binary doubles, the webview assets are shared.
- Updates arrive through `brew upgrade --cask`. CI bumps the cask's version and
  checksum on every tag, so there is no manual release step — but nobody is
  *told* an update exists. Tauri's own updater was rejected as a second update
  path that can disagree with Homebrew's, plus a private key to not lose.

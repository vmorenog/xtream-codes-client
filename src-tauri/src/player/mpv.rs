//! Playback, delegated to an mpv sidecar over its JSON IPC socket (ADR-0002).
//!
//! mpv is spawned idle with no file argument. The **Stream URL** is sent later
//! over the socket, never on the command line — a command line is visible to
//! every process on the machine via `ps`, and the URL carries the **Provider**
//! password.

use std::path::PathBuf;
use std::process::Stdio;

use serde::Serialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout, Duration};

use crate::error::{AppError, Result};

/// Where Homebrew puts mpv, plus whatever is on PATH (ADR-0003).
const MPV_CANDIDATES: &[&str] = &[
    "/opt/homebrew/bin/mpv",
    "/usr/local/bin/mpv",
    "/usr/bin/mpv",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStatus {
    pub running: bool,
    pub playing: bool,
    pub paused: bool,
    /// Seconds into the current **Playable**. Meaningless for a **Channel**.
    pub position_secs: Option<i64>,
    pub duration_secs: Option<i64>,
    pub title: Option<String>,
}

pub struct Player {
    inner: Mutex<Option<Running>>,
    socket_path: PathBuf,
}

struct Running {
    child: Child,
    title: Option<String>,
}

impl Player {
    pub fn new(runtime_dir: &std::path::Path) -> Self {
        Self {
            inner: Mutex::new(None),
            socket_path: runtime_dir.join("mpv.sock"),
        }
    }

    /// Locates mpv, or explains how to install it.
    pub fn locate() -> Result<PathBuf> {
        for c in MPV_CANDIDATES {
            let p = PathBuf::from(c);
            if p.is_file() {
                return Ok(p);
            }
        }
        // Fall back to PATH for anyone not using Homebrew's prefix.
        if let Ok(out) = std::process::Command::new("/usr/bin/which")
            .arg("mpv")
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Ok(PathBuf::from(s));
                }
            }
        }
        Err(AppError::MpvMissing)
    }

    pub fn is_installed() -> bool {
        Self::locate().is_ok()
    }

    /// Starts mpv idle if it is not already up, and waits for its IPC socket.
    async fn ensure_running(&self) -> Result<()> {
        let mut guard = self.inner.lock().await;

        if let Some(running) = guard.as_mut() {
            match running.child.try_wait() {
                Ok(None) => return Ok(()), // still alive
                _ => {
                    *guard = None; // exited; fall through and respawn
                }
            }
        }

        let bin = Self::locate()?;
        let _ = std::fs::remove_file(&self.socket_path);
        if let Some(dir) = self.socket_path.parent() {
            std::fs::create_dir_all(dir)?;
        }

        let child = Command::new(bin)
            .arg("--idle=yes")
            .arg("--force-window=yes")
            .arg("--no-terminal")
            .arg("--keep-open=no")
            .arg("--save-position-on-quit=no")
            .arg("--osc=yes")
            // Live TS has no seekable index; a large cache is what stops the
            // stutter that makes IPTV feel broken.
            .arg("--cache=yes")
            .arg("--demuxer-max-bytes=64MiB")
            .arg("--demuxer-readahead-secs=20")
            .arg("--hwdec=videotoolbox")
            .arg(format!("--input-ipc-server={}", self.socket_path.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;

        *guard = Some(Running { child, title: None });
        drop(guard);

        // mpv creates the socket a beat after exec.
        for _ in 0..50 {
            if self.socket_path.exists() && UnixStream::connect(&self.socket_path).await.is_ok() {
                return Ok(());
            }
            sleep(Duration::from_millis(40)).await;
        }
        Err(AppError::MpvUnresponsive(
            "socket never appeared after 2s".into(),
        ))
    }

    /// Sends one command and reads the reply.
    ///
    /// A fresh connection per command: mpv's socket multiplexes replies with
    /// unsolicited events, and one-shot connections sidestep having to
    /// demultiplex them for the handful of commands we send.
    async fn command(&self, args: Value) -> Result<Value> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .map_err(|e| AppError::MpvUnresponsive(e.to_string()))?;

        let payload = format!("{}\n", json!({ "command": args }));
        stream
            .write_all(payload.as_bytes())
            .await
            .map_err(|e| AppError::MpvUnresponsive(e.to_string()))?;

        let mut reader = BufReader::new(stream);
        let deadline = Duration::from_secs(5);
        loop {
            let mut line = String::new();
            let n = timeout(deadline, reader.read_line(&mut line))
                .await
                .map_err(|_| AppError::MpvUnresponsive("timed out waiting for a reply".into()))?
                .map_err(|e| AppError::MpvUnresponsive(e.to_string()))?;
            if n == 0 {
                return Err(AppError::MpvUnresponsive("socket closed".into()));
            }
            let Ok(v): std::result::Result<Value, _> = serde_json::from_str(&line) else {
                continue;
            };
            // Skip the event stream; we only want the reply to our command.
            if v.get("event").is_some() {
                continue;
            }
            if v.get("error").and_then(Value::as_str) == Some("success") {
                return Ok(v.get("data").cloned().unwrap_or(Value::Null));
            }
            return Err(AppError::MpvUnresponsive(
                v.get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown error")
                    .to_string(),
            ));
        }
    }

    /// Loads a **Stream URL**.
    ///
    /// `url` is a secret and is deliberately taken by value so it is dropped as
    /// soon as it has been written to the socket. Do not log it, do not return
    /// it, do not store it.
    pub async fn play(
        &self,
        url: String,
        title: Option<String>,
        start_at: Option<i64>,
    ) -> Result<()> {
        self.ensure_running().await?;

        let mut opts = serde_json::Map::new();
        if let Some(secs) = start_at.filter(|s| *s > 0) {
            opts.insert("start".into(), json!(format!("+{secs}")));
        }
        if let Some(t) = &title {
            opts.insert("force-media-title".into(), json!(t));
        }

        tracing::info!(
            title = title.as_deref().unwrap_or("<untitled>"),
            "loading playable"
        );

        self.command(json!(["loadfile", url, "replace", 0, Value::Object(opts)]))
            .await?;

        if let Some(running) = self.inner.lock().await.as_mut() {
            running.title = title;
        }
        Ok(())
    }

    pub async fn toggle_pause(&self) -> Result<()> {
        self.command(json!(["cycle", "pause"])).await?;
        Ok(())
    }

    pub async fn seek(&self, seconds: i64) -> Result<()> {
        self.command(json!(["seek", seconds, "relative"])).await?;
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let mut guard = self.inner.lock().await;
        if let Some(mut running) = guard.take() {
            // Ask nicely first so mpv tears down its window cleanly.
            let quit = timeout(
                Duration::from_secs(2),
                self.command_unlocked(json!(["quit"])),
            )
            .await;
            if quit.is_err() {
                let _ = running.child.kill().await;
            }
            let _ = running.child.wait().await;
        }
        let _ = std::fs::remove_file(&self.socket_path);
        Ok(())
    }

    async fn command_unlocked(&self, args: Value) -> Result<Value> {
        // `stop` already holds the process lock; `command` does not take it.
        self.command(args).await
    }

    pub async fn status(&self) -> PlayerStatus {
        let (running, title) = {
            let mut guard = self.inner.lock().await;
            match guard.as_mut() {
                Some(r) => (matches!(r.child.try_wait(), Ok(None)), r.title.clone()),
                None => (false, None),
            }
        };

        if !running {
            return PlayerStatus {
                running: false,
                playing: false,
                paused: false,
                position_secs: None,
                duration_secs: None,
                title: None,
            };
        }

        let position = self.property_i64("time-pos").await;
        let duration = self.property_i64("duration").await;
        let paused = self
            .command(json!(["get_property", "pause"]))
            .await
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let idle = self
            .command(json!(["get_property", "idle-active"]))
            .await
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        PlayerStatus {
            running: true,
            playing: !idle,
            paused,
            position_secs: position,
            duration_secs: duration,
            title,
        }
    }

    async fn property_i64(&self, name: &str) -> Option<i64> {
        self.command(json!(["get_property", name]))
            .await
            .ok()
            .and_then(|v| v.as_f64())
            // Live streams report a NaN/absurd duration; treat those as absent.
            .filter(|f| f.is_finite() && *f >= 0.0)
            .map(|f| f as i64)
    }
}

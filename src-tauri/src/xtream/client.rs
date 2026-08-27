use std::time::Duration;

use serde::de::DeserializeOwned;
use url::Url;

use super::model::*;
use super::{CatalogueKind, PlayableKind};
use crate::error::{AppError, Result};

/// Talks to one **Provider**'s panel.
///
/// Every method here sends the username and password. Nothing in this module
/// may log a constructed URL — see ADR-0002.
pub struct XtreamClient {
    http: reqwest::Client,
    base: Url,
    username: String,
    password: String,
}

impl XtreamClient {
    pub fn new(base_url: &str, username: &str, password: &str) -> Result<Self> {
        let base = normalise_base(base_url)?;
        let http = reqwest::Client::builder()
            // Panels are slow and the catalogue endpoints return tens of MB.
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(15))
            .user_agent("VLC/3.0.20 LibVLC/3.0.20")
            .build()
            .map_err(|e| AppError::Other(e.to_string()))?;

        Ok(Self {
            http,
            base,
            username: username.to_string(),
            password: password.to_string(),
        })
    }

    /// Confirms the credentials and reads the **Entitlement**.
    pub async fn handshake(&self) -> Result<Handshake> {
        let res: Handshake = self.call(None, &[]).await?;
        if !res.user_info.auth {
            return Err(AppError::BadCredentials);
        }
        match res.user_info.status.as_deref() {
            Some("Active") | None => Ok(res),
            Some(other) => Err(AppError::NotEntitled(other.to_string())),
        }
    }

    pub async fn categories(&self, kind: CatalogueKind) -> Result<Vec<RawCategory>> {
        self.call(Some(kind.category_action()), &[]).await
    }

    pub async fn channels(&self) -> Result<Vec<RawChannel>> {
        self.call(Some("get_live_streams"), &[]).await
    }

    pub async fn movies(&self) -> Result<Vec<RawMovie>> {
        self.call(Some("get_vod_streams"), &[]).await
    }

    pub async fn series(&self) -> Result<Vec<RawSeries>> {
        self.call(Some("get_series"), &[]).await
    }

    pub async fn series_info(&self, series_id: i64) -> Result<RawSeriesInfo> {
        self.call(
            Some("get_series_info"),
            &[("series_id", series_id.to_string())],
        )
        .await
    }

    /// The next `limit` **Programmes** on a **Channel**.
    pub async fn short_schedule(&self, stream_id: i64, limit: u32) -> Result<Vec<RawProgramme>> {
        let res: RawScheduleResponse = self
            .call(
                Some("get_short_epg"),
                &[
                    ("stream_id", stream_id.to_string()),
                    ("limit", limit.to_string()),
                ],
            )
            .await?;
        Ok(res.epg_listings)
    }

    /// Builds the credential-bearing **Stream URL**.
    ///
    /// The return value is a secret: it contains the **Provider** password in
    /// its path. Hand it straight to mpv over the IPC socket and drop it. Never
    /// log it, never return it to the webview, never put it on a command line.
    pub fn stream_url(&self, kind: PlayableKind, id: &str, extension: Option<&str>) -> Result<Url> {
        let ext = extension.unwrap_or(match kind {
            PlayableKind::Channel => "ts",
            _ => "mp4",
        });
        let path = format!(
            "{}/{}/{}/{}.{}",
            kind.url_segment(),
            self.username,
            self.password,
            id,
            ext
        );
        self.base
            .join(&path)
            .map_err(|e| AppError::Other(e.to_string()))
    }

    async fn call<T: DeserializeOwned>(
        &self,
        action: Option<&str>,
        params: &[(&str, String)],
    ) -> Result<T> {
        let mut url = self
            .base
            .join("player_api.php")
            .map_err(|e| AppError::Other(e.to_string()))?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("username", &self.username);
            q.append_pair("password", &self.password);
            if let Some(a) = action {
                q.append_pair("action", a);
            }
            for (k, v) in params {
                q.append_pair(k, v);
            }
        }

        tracing::debug!(action = action.unwrap_or("handshake"), "calling provider");

        let res = self.http.get(url).send().await?;
        let status = res.status();
        if !status.is_success() {
            return Err(AppError::Unreachable(format!("HTTP {}", status.as_u16())));
        }

        // Panels sometimes answer a valid request with an HTML error page or a
        // bare `false`. Read to bytes first so the error says something useful.
        let bytes = res.bytes().await?;
        serde_json::from_slice(&bytes).map_err(|e| {
            let head: String = String::from_utf8_lossy(&bytes).chars().take(120).collect();
            AppError::BadResponse(format!("{e} (starts with: {head:?})"))
        })
    }
}

/// Accepts `host:8080`, `http://host:8080`, `http://host:8080/` and
/// `http://host:8080/c/` and normalises to a base with a trailing slash so
/// `Url::join` behaves.
fn normalise_base(input: &str) -> Result<Url> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AppError::Other("base URL is empty".into()));
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    let mut url =
        Url::parse(&with_scheme).map_err(|_| AppError::Other("base URL is not valid".into()))?;
    if !url.path().ends_with('/') {
        let p = format!("{}/", url.path());
        url.set_path(&p);
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_accepts_a_bare_host_and_port() {
        let u = normalise_base("example.com:8080").unwrap();
        assert_eq!(u.as_str(), "http://example.com:8080/");
    }

    #[test]
    fn base_url_keeps_https_and_adds_a_trailing_slash() {
        let u = normalise_base("https://example.com:8443").unwrap();
        assert_eq!(u.as_str(), "https://example.com:8443/");
    }

    #[test]
    fn base_url_preserves_a_subpath() {
        let u = normalise_base("http://example.com/panel").unwrap();
        assert_eq!(u.as_str(), "http://example.com/panel/");
    }

    #[test]
    fn stream_url_puts_credentials_in_the_path() {
        let c = XtreamClient::new("http://example.com:8080", "bob", "hunter2").unwrap();
        let u = c.stream_url(PlayableKind::Channel, "42", None).unwrap();
        assert_eq!(u.as_str(), "http://example.com:8080/live/bob/hunter2/42.ts");
    }

    #[test]
    fn stream_url_honours_the_container_extension() {
        let c = XtreamClient::new("example.com", "bob", "hunter2").unwrap();
        let u = c.stream_url(PlayableKind::Movie, "7", Some("mkv")).unwrap();
        assert!(u.as_str().ends_with("/movie/bob/hunter2/7.mkv"));
    }

    #[test]
    fn channels_have_no_resume_point() {
        assert!(!PlayableKind::Channel.resumable());
        assert!(PlayableKind::Movie.resumable());
        assert!(PlayableKind::Episode.resumable());
    }
}

//! Daily release checks; no installation identifier or desktop information is sent.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use inset_foundation::{App, Context, Listener, Timer};
use semver::Version;
use serde::Deserialize;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const ENDPOINT: &str = "https://edged2.app/api/version";
const DOWNLOAD_PAGE: &str = "https://edged2.app/download";
const AUTOMATIC_KEY: &str = "automatic_updates";
const LAST_CHECK_KEY: &str = "update_check_attempt";
const LATEST_KEY: &str = "latest_version";
const DAY: u64 = 86_400;

#[derive(Deserialize)]
struct Release {
    version: String,
}

/// Update state is visible in Settings and the panel menu; installation stays user-controlled.
pub struct Updates {
    pub automatic: bool,
    pub checking: bool,
    /// Whether this launch has received a valid release response.
    pub checked: bool,
    pub latest: Option<String>,
    pub error: Option<String>,
    last_attempt: Option<u64>,
    timer: Option<Timer>,
}

impl Updates {
    pub fn start(cx: &mut Context<Self>) -> Self {
        let mut updates = Self {
            automatic: edged_macos::read_default(AUTOMATIC_KEY).as_deref() != Some("off"),
            checking: false,
            checked: false,
            latest: edged_macos::read_default(LATEST_KEY)
                .filter(|version| newer_version(CURRENT_VERSION, version)),
            error: None,
            last_attempt: edged_macos::read_default(LAST_CHECK_KEY)
                .and_then(|value| value.parse().ok()),
            timer: None,
        };
        updates.schedule(cx, Duration::from_secs(10));
        updates
    }

    /// Automatic checks are limited across restarts, including unsuccessful attempts.
    fn schedule(&mut self, cx: &mut Context<Self>, delay: Duration) {
        let this = cx.weak_entity();
        self.timer = Some(Timer::new(
            cx,
            delay,
            Listener::new(move |app: &mut App| {
                if let Some(updates) = this.upgrade() {
                    updates.update(app, |updates, cx| {
                        updates.timer = None;
                        if updates.automatic && due(updates.last_attempt, now()) {
                            updates.start_check(cx, false);
                        }
                        updates.schedule(cx, Duration::from_secs(3600));
                    });
                }
            }),
        ));
    }

    /// Manual checks bypass the daily limit but never start overlapping requests.
    pub fn check(&mut self, cx: &mut Context<Self>) {
        self.start_check(cx, true);
    }

    /// Background failures stay quiet; only an explicit check reports an error.
    fn start_check(&mut self, cx: &mut Context<Self>, report_error: bool) {
        if self.checking {
            return;
        }
        let timestamp = now();
        self.last_attempt = Some(timestamp);
        edged_macos::write_default(LAST_CHECK_KEY, &timestamp.to_string());
        self.checking = true;
        self.checked = false;
        self.error = None;
        cx.notify();

        let this = cx.weak_entity();
        cx.spawn(async move |cx| {
            let result = cx.run_in_background(|| fetch_release(ENDPOINT)).await;
            let Some(updates) = this.upgrade() else {
                return;
            };
            cx.update(|app| {
                updates.update(app, |updates, cx| {
                    updates.finish_check(cx, result, report_error);
                });
            });
        });
    }

    fn finish_check(
        &mut self,
        cx: &mut Context<Self>,
        result: Result<String, String>,
        report_error: bool,
    ) {
        self.checking = false;
        match result.and_then(|body| parse_release(&body)) {
            Ok(version) => {
                self.checked = true;
                edged_macos::write_default(LATEST_KEY, &version);
                self.latest = newer_version(CURRENT_VERSION, &version).then_some(version);
            }
            Err(_) if report_error => {
                self.error = Some("Could not check for updates. Try again later.".into());
            }
            Err(_) => {}
        }
        cx.notify();
    }

    pub fn set_automatic(&mut self, cx: &mut Context<Self>, enabled: bool) {
        self.automatic = enabled;
        edged_macos::write_default(AUTOMATIC_KEY, if enabled { "on" } else { "off" });
        cx.notify();
        if enabled && due(self.last_attempt, now()) {
            self.start_check(cx, false);
        }
    }

    pub fn download(&mut self, cx: &mut Context<Self>) {
        if edged_macos::open_url(DOWNLOAD_PAGE).is_err() {
            self.error = Some("Could not open the download page.".into());
            cx.notify();
        }
    }
}

/// The endpoint's answer, read on the calling thread: at most 16 KiB, within 15 seconds.
fn fetch_release(url: &str) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    agent
        .get(url)
        .header("User-Agent", concat!("Edged2/", env!("CARGO_PKG_VERSION")))
        .call()
        .and_then(|mut response| {
            response
                .body_mut()
                .with_config()
                .limit(16 * 1024)
                .read_to_string()
        })
        .map_err(|error| error.to_string())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn due(last: Option<u64>, current: u64) -> bool {
    last.is_none_or(|last| current < last || current / DAY > last / DAY)
}

fn parse_release(body: &str) -> Result<String, String> {
    let release: Release = serde_json::from_str(body).map_err(|error| error.to_string())?;
    let version = Version::parse(&release.version).map_err(|error| error.to_string())?;
    if !version.pre.is_empty() {
        return Err("Expected a stable release".into());
    }
    Ok(version.to_string())
}

fn newer_version(current: &str, available: &str) -> bool {
    match (Version::parse(current), Version::parse(available)) {
        (Ok(current), Ok(available)) => {
            available.pre.is_empty() && available.cmp_precedence(&current).is_gt()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daily_checks_survive_restarts_and_handle_clock_changes() {
        assert!(due(None, 100));
        assert!(!due(Some(100), 200));
        assert!(due(Some(100), DAY));
        assert!(due(Some(200), 100));
    }

    #[test]
    fn versions_are_compared_numerically_and_never_downgraded() {
        assert!(newer_version("0.9.0", "0.10.0"));
        assert!(!newer_version("0.10.0", "0.9.0"));
        assert!(!newer_version("0.10.0", "0.10.0"));
        assert!(!newer_version("0.10.0", "0.11.0-beta.1"));
        assert!(!newer_version("0.10.0", "invalid"));
        assert!(!newer_version("0.10.0+one", "0.10.0+two"));
    }

    #[test]
    fn malformed_or_prerelease_responses_are_rejected() {
        assert_eq!(parse_release(r#"{"version":"0.2.0"}"#).unwrap(), "0.2.0");
        assert!(parse_release(r#"{"version":"0.3.0-beta"}"#).is_err());
        assert!(parse_release(r#"{"error":"unavailable"}"#).is_err());
    }
}

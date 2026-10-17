//! Opt-out notice that a newer Nahuali release exists.
//!
//! This is the only network call Nahuali makes on its own: a plain GET of the
//! public GitHub releases list, at most once per 24 hours, bounded by a short
//! timeout. The outcome, success or failure, is cached under the Nahuali home so
//! an offline machine pays one attempt per day, not one per command. Nothing is
//! ever downloaded or installed; the operator runs the printed command.

use std::{
    cmp::Ordering,
    fmt, fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use serde::{Deserialize, Serialize};

const RELEASES_API: &str = "https://api.github.com/repos/Arakiss/nahuali/releases?per_page=30";
const RELEASE_PAGE: &str = "https://github.com/Arakiss/nahuali/releases/tag";
const INSTALLER_BASE: &str = "https://raw.githubusercontent.com/Arakiss/nahuali";
/// Replaces the releases URL so tests can serve a fake release list locally.
const API_URL_ENV: &str = "NAHUALI_UPDATE_CHECK_URL";
const OPT_OUT_ENV: &str = "NAHUALI_NO_UPDATE_CHECK";
const CACHE_FILE: &str = "update-check.json";
const CHECK_INTERVAL_MS: u64 = 24 * 60 * 60 * 1000;
const TIMEOUT: Duration = Duration::from_millis(1500);

/// Whether an update check may touch the network.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gate {
    Enabled,
    /// `NAHUALI_NO_UPDATE_CHECK` is set: no network call, ever.
    OptedOut,
    /// `CI` is set: the automatic notice is skipped; an explicit
    /// `nahuali update-check` still runs.
    Ci,
}

fn gate(lookup: impl Fn(&str) -> Option<String>) -> Gate {
    if is_truthy(lookup(OPT_OUT_ENV)) {
        Gate::OptedOut
    } else if is_truthy(lookup("CI")) {
        Gate::Ci
    } else {
        Gate::Enabled
    }
}

fn is_truthy(value: Option<String>) -> bool {
    value.is_some_and(|value| {
        let value = value.trim();
        !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
    })
}

fn env_gate() -> Gate {
    gate(|name| std::env::var(name).ok())
}

/// A semantic version ordered by SemVer 2.0 precedence, so `0.8.0-beta.9`
/// sorts before `0.8.0-beta.13` and a prerelease before its stable release.
/// Build metadata is accepted and ignored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    pre: Vec<PreId>,
}

/// Numeric identifiers sort before alphanumeric ones, as SemVer requires.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PreId {
    Numeric(u64),
    Alpha(String),
}

impl Version {
    fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        let raw = raw.strip_prefix('v').unwrap_or(raw);
        let raw = raw.split_once('+').map_or(raw, |(version, _build)| version);
        let (core, pre) = match raw.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (raw, None),
        };
        let mut numbers = core.split('.').map(|part| part.parse::<u64>().ok());
        let (major, minor, patch) = (numbers.next()??, numbers.next()??, numbers.next()??);
        if numbers.next().is_some() {
            return None;
        }
        let pre = match pre {
            None => Vec::new(),
            Some(pre) => pre
                .split('.')
                .map(|id| {
                    if id.is_empty() {
                        None
                    } else if id.bytes().all(|byte| byte.is_ascii_digit()) {
                        id.parse().ok().map(PreId::Numeric)
                    } else {
                        Some(PreId::Alpha(id.to_string()))
                    }
                })
                .collect::<Option<Vec<_>>>()?,
        };
        Some(Self {
            major,
            minor,
            patch,
            pre,
        })
    }

    fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => self.pre.cmp(&other.pre),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)?;
        for (index, id) in self.pre.iter().enumerate() {
            formatter.write_str(if index == 0 { "-" } else { "." })?;
            match id {
                PreId::Numeric(number) => write!(formatter, "{number}")?,
                PreId::Alpha(text) => formatter.write_str(text)?,
            }
        }
        Ok(())
    }
}

fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("the package version is SemVer")
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
}

/// The newest release worth offering, mirroring `scripts/install.sh`: only
/// `v*` product tags that ship a binary archive for this platform count, so a
/// tag carrying just an SBOM is never offered. Releases are prereleases, so
/// `/releases/latest` cannot be used. A stable build is offered only stable
/// releases; a prerelease build follows the prerelease train.
fn newest_release(
    releases: &[Release],
    target: Option<&str>,
    include_prereleases: bool,
) -> Option<Version> {
    let archive_suffix = target.map_or_else(|| ".tar.gz".to_string(), |t| format!("-{t}.tar.gz"));
    releases
        .iter()
        .filter(|release| !release.draft && release.tag_name.starts_with('v'))
        .filter(|release| {
            release
                .assets
                .iter()
                .any(|asset| asset.name.ends_with(&archive_suffix))
        })
        .filter_map(|release| Version::parse(&release.tag_name))
        .filter(|version| include_prereleases || !version.is_prerelease())
        .max()
}

/// The release target triple this binary would install, as `install.sh`
/// names it, or `None` on a platform without prebuilt archives.
fn host_target() -> Option<&'static str> {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "macos") => Some("x86_64-apple-darwin"),
        ("aarch64", "macos") => Some("aarch64-apple-darwin"),
        ("x86_64", "linux") => Some("x86_64-unknown-linux-gnu"),
        ("aarch64", "linux") => Some("aarch64-unknown-linux-gnu"),
        _ => None,
    }
}

fn releases_url() -> String {
    std::env::var(API_URL_ENV)
        .ok()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| RELEASES_API.to_string())
}

/// One anonymous GET; the User-Agent GitHub requires carries only the version.
fn fetch_newest(url: &str, current: &Version) -> anyhow::Result<Option<Version>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .connect_timeout(TIMEOUT)
        .user_agent(concat!("nahuali/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("could not build the HTTP client")?;
    let releases: Vec<Release> = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .context("could not reach the GitHub releases API")?
        .json()
        .context("the GitHub releases API returned an unexpected response")?;
    Ok(newest_release(
        &releases,
        host_target(),
        current.is_prerelease(),
    ))
}

#[derive(Debug, Deserialize, Serialize)]
struct CacheEntry {
    checked_at_ms: u64,
    latest_version: Option<String>,
}

impl CacheEntry {
    fn latest(&self) -> Option<Version> {
        self.latest_version.as_deref().and_then(Version::parse)
    }

    /// Fresh for 24 hours; a clock that moved backwards counts as stale.
    fn is_fresh(&self, now_ms: u64) -> bool {
        now_ms
            .checked_sub(self.checked_at_ms)
            .is_some_and(|age| age < CHECK_INTERVAL_MS)
    }
}

fn cache_path(home: &Path) -> PathBuf {
    home.join(CACHE_FILE)
}

fn read_cache(path: &Path) -> Option<CacheEntry> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

/// Best effort and atomic: concurrent sessions never read a torn file, and a
/// read-only home only means the next command checks again.
fn write_cache(path: &Path, entry: &CacheEntry) {
    let Ok(body) = serde_json::to_vec_pretty(entry) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let staging = path.with_extension(format!("json.{}.tmp", std::process::id()));
    if fs::write(&staging, body).is_ok() && fs::rename(&staging, path).is_err() {
        let _ = fs::remove_file(&staging);
    }
}

/// Cache the outcome of a check and return it. A failed check is still a
/// check: it keeps the last known release and waits a day before retrying.
fn record_check(
    home: &Path,
    now_ms: u64,
    outcome: anyhow::Result<Option<Version>>,
) -> anyhow::Result<Option<Version>> {
    let path = cache_path(home);
    let latest = match &outcome {
        Ok(latest) => latest.clone(),
        Err(_) => read_cache(&path).and_then(|entry| entry.latest()),
    };
    write_cache(
        &path,
        &CacheEntry {
            checked_at_ms: now_ms,
            latest_version: latest.as_ref().map(ToString::to_string),
        },
    );
    outcome
}

/// The newest known release for the automatic notice: the cached answer while
/// it is fresh, otherwise one bounded fetch whose outcome is cached.
fn newest_for_notice(
    home: &Path,
    now_ms: u64,
    fetch: impl FnOnce() -> anyhow::Result<Option<Version>>,
) -> Option<Version> {
    let cached = read_cache(&cache_path(home));
    if let Some(entry) = &cached
        && entry.is_fresh(now_ms)
    {
        return entry.latest();
    }
    match record_check(home, now_ms, fetch()) {
        Ok(latest) => latest,
        Err(_) => cached.and_then(|entry| entry.latest()),
    }
}

fn notice_for(current: &Version, latest: &Version) -> Option<String> {
    (latest > current).then(|| {
        format!("Update available: {latest} (you have {current}) · {RELEASE_PAGE}/v{latest}")
    })
}

fn upgrade_command(latest: &Version) -> String {
    format!(
        "curl -fsSLo /tmp/nahuali-install.sh {INSTALLER_BASE}/v{latest}/scripts/install.sh && NAHUALI_VERSION=v{latest} sh /tmp/nahuali-install.sh"
    )
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// The one-line notice for human output, or `None` when the build is current,
/// the check is disabled, or the network gave no answer. Blocks for at most
/// the fetch timeout, and only once per day.
pub(crate) fn notice_line() -> Option<String> {
    if env_gate() != Gate::Enabled {
        return None;
    }
    let current = current_version();
    let latest = newest_for_notice(&nahuali_core::nahuali_home(), now_ms(), || {
        fetch_newest(&releases_url(), &current)
    })?;
    notice_for(&current, &latest)
}

/// `nahuali update-check`: check now, ignoring the daily cache, and print the
/// pinned upgrade command. Only `NAHUALI_NO_UPDATE_CHECK` stops it.
pub(crate) fn update_check(json: bool) -> anyhow::Result<()> {
    let current = current_version();
    if env_gate() == Gate::OptedOut {
        if json {
            crate::output::print_json(&serde_json::json!({
                "checked": false,
                "reason": format!("{OPT_OUT_ENV} is set"),
                "current_version": current.to_string(),
            }))?;
        } else {
            println!("Update check disabled by {OPT_OUT_ENV}; no network call made.");
        }
        return Ok(());
    }

    let outcome = fetch_newest(&releases_url(), &current);
    let latest = record_check(&nahuali_core::nahuali_home(), now_ms(), outcome)?;
    let update = latest.as_ref().filter(|latest| *latest > &current);
    if json {
        crate::output::print_json(&serde_json::json!({
            "checked": true,
            "current_version": current.to_string(),
            "latest_version": latest.as_ref().map(ToString::to_string),
            "update_available": update.is_some(),
            "release_url": update.map(|latest| format!("{RELEASE_PAGE}/v{latest}")),
            "upgrade_command": update.map(upgrade_command),
        }))?;
        return Ok(());
    }
    match (&latest, update) {
        (_, Some(latest)) => {
            if let Some(line) = notice_for(&current, latest) {
                println!("{line}");
            }
            println!("Upgrade with:\n  {}", upgrade_command(latest));
            println!("Nahuali never installs updates on its own.");
        }
        (Some(latest), None) => {
            println!("Nahuali {current} is up to date (newest release: {latest}).");
        }
        (None, None) => {
            println!("Nahuali {current}: no published release for this platform was found.");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(raw: &str) -> Version {
        Version::parse(raw).unwrap_or_else(|| panic!("{raw} parses"))
    }

    fn release(tag: &str, assets: &[&str]) -> Release {
        Release {
            tag_name: tag.to_string(),
            draft: false,
            assets: assets
                .iter()
                .map(|name| Asset {
                    name: (*name).to_string(),
                })
                .collect(),
        }
    }

    fn temp_home(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "nahuali-update-check-{name}-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let _ = fs::remove_dir_all(&path);
        path
    }

    const DAY_MS: u64 = CHECK_INTERVAL_MS;

    #[test]
    fn orders_numeric_prerelease_identifiers_numerically() {
        assert!(version("0.8.0-beta.9") < version("0.8.0-beta.13"));
        assert!(version("v0.8.0-beta.13") > version("0.8.0-beta.10"));
        assert_eq!(version("v0.8.0-beta.13"), version("0.8.0-beta.13"));
        assert!(version("0.8.0-beta.13") < version("0.8.1-beta.1"));
        assert!(version("0.10.0-beta.1") > version("0.9.0"));
        assert!(version("0.10.0") > version("0.9.99"));
    }

    #[test]
    fn a_prerelease_sorts_before_its_stable_release() {
        assert!(version("0.8.0-beta.13") < version("0.8.0"));
        assert!(version("0.8.0") < version("0.8.1-beta.1"));
        assert!(version("1.0.0-alpha") < version("1.0.0-alpha.1"));
        assert!(version("1.0.0-alpha.1") < version("1.0.0-alpha.beta"));
        assert!(version("1.0.0-beta.11") < version("1.0.0-rc.1"));
        assert_eq!(version("1.0.0+build.7"), version("1.0.0"));
    }

    #[test]
    fn rejects_tags_that_are_not_versions() {
        for raw in [
            "",
            "nahuali-v0.8.0",
            "0.8",
            "0.8.0.1",
            "0.8.0-",
            "0.8.0-beta..1",
            "x.y.z",
        ] {
            assert!(Version::parse(raw).is_none(), "{raw:?} must not parse");
        }
    }

    #[test]
    fn displays_the_normalized_version() {
        assert_eq!(version("v0.8.0-beta.13+abc").to_string(), "0.8.0-beta.13");
        assert_eq!(version("1.2.3").to_string(), "1.2.3");
    }

    #[test]
    fn newest_release_needs_a_product_tag_with_an_archive_for_this_target() {
        let target = "aarch64-apple-darwin";
        let releases = [
            // Newest tag but SBOM-only, like the workspace-root release.
            release("nahuali-v0.8.0-beta.15", &["nahuali.cdx.json"]),
            // Product tag still uploading: no archive for this target yet.
            release(
                "v0.8.0-beta.14",
                &["nahuali-v0.8.0-beta.14-x86_64-apple-darwin.tar.gz"],
            ),
            release(
                "v0.8.0-beta.13",
                &[
                    "nahuali-v0.8.0-beta.13-aarch64-apple-darwin.tar.gz",
                    "nahuali-v0.8.0-beta.13-aarch64-apple-darwin.tar.gz.sha256",
                ],
            ),
            release(
                "v0.8.0-beta.9",
                &["nahuali-v0.8.0-beta.9-aarch64-apple-darwin.tar.gz"],
            ),
        ];
        assert_eq!(
            newest_release(&releases, Some(target), true),
            Some(version("0.8.0-beta.13"))
        );
        assert_eq!(
            newest_release(&releases, None, true),
            Some(version("0.8.0-beta.14"))
        );
    }

    #[test]
    fn newest_release_skips_drafts_and_offers_stable_builds_only_stable_releases() {
        let mut draft = release(
            "v0.9.0",
            &["nahuali-v0.9.0-x86_64-unknown-linux-gnu.tar.gz"],
        );
        draft.draft = true;
        let releases = [
            draft,
            release(
                "v0.8.1-beta.1",
                &["nahuali-v0.8.1-beta.1-x86_64-unknown-linux-gnu.tar.gz"],
            ),
            release(
                "v0.8.0",
                &["nahuali-v0.8.0-x86_64-unknown-linux-gnu.tar.gz"],
            ),
        ];
        let target = Some("x86_64-unknown-linux-gnu");
        assert_eq!(
            newest_release(&releases, target, true),
            Some(version("0.8.1-beta.1"))
        );
        assert_eq!(
            newest_release(&releases, target, false),
            Some(version("0.8.0"))
        );
        assert_eq!(newest_release(&[], target, true), None);
    }

    #[test]
    fn parses_the_github_release_list_shape() {
        let body = r#"[{"tag_name":"v0.8.0-beta.13","draft":false,"prerelease":true,
            "assets":[{"name":"nahuali-v0.8.0-beta.13-x86_64-apple-darwin.tar.gz","size":1}]}]"#;
        let releases: Vec<Release> = serde_json::from_str(body).expect("release list parses");
        assert_eq!(
            newest_release(&releases, Some("x86_64-apple-darwin"), true),
            Some(version("0.8.0-beta.13"))
        );
    }

    #[test]
    fn notice_only_when_the_release_is_newer() {
        let current = version("0.8.0-beta.13");
        assert_eq!(
            notice_for(&current, &version("0.8.0-beta.14")).as_deref(),
            Some(
                "Update available: 0.8.0-beta.14 (you have 0.8.0-beta.13) · https://github.com/Arakiss/nahuali/releases/tag/v0.8.0-beta.14"
            )
        );
        assert_eq!(notice_for(&current, &version("0.8.0-beta.13")), None);
        assert_eq!(notice_for(&current, &version("0.8.0-beta.9")), None);
    }

    #[test]
    fn upgrade_command_pins_the_release() {
        assert_eq!(
            upgrade_command(&version("0.8.0-beta.14")),
            "curl -fsSLo /tmp/nahuali-install.sh https://raw.githubusercontent.com/Arakiss/nahuali/v0.8.0-beta.14/scripts/install.sh && NAHUALI_VERSION=v0.8.0-beta.14 sh /tmp/nahuali-install.sh"
        );
    }

    #[test]
    fn opt_out_wins_and_ci_only_skips_the_automatic_check() {
        let lookup = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_string())
            }
        };
        assert_eq!(gate(lookup(&[])), Gate::Enabled);
        assert_eq!(
            gate(lookup(&[("NAHUALI_NO_UPDATE_CHECK", "1")])),
            Gate::OptedOut
        );
        assert_eq!(
            gate(lookup(&[("NAHUALI_NO_UPDATE_CHECK", "1"), ("CI", "true")])),
            Gate::OptedOut
        );
        assert_eq!(gate(lookup(&[("CI", "true")])), Gate::Ci);
        assert_eq!(gate(lookup(&[("CI", "1")])), Gate::Ci);
        for off in ["", " ", "0", "false", "FALSE"] {
            let lookup = |name: &str| {
                matches!(name, "NAHUALI_NO_UPDATE_CHECK" | "CI").then(|| off.to_string())
            };
            assert_eq!(gate(lookup), Gate::Enabled, "{off:?} does not disable");
        }
    }

    #[test]
    fn a_fresh_cache_answers_without_fetching() {
        let home = temp_home("fresh");
        let now = 10 * DAY_MS;
        record_check(&home, now - DAY_MS + 1, Ok(Some(version("0.8.0-beta.14")))).unwrap();

        let latest = newest_for_notice(&home, now, || panic!("a fresh cache must not fetch"));
        assert_eq!(latest, Some(version("0.8.0-beta.14")));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn a_stale_cache_fetches_once_and_records_the_answer() {
        let home = temp_home("stale");
        let now = 10 * DAY_MS;
        record_check(&home, now - DAY_MS, Ok(Some(version("0.8.0-beta.13")))).unwrap();

        let mut fetches = 0;
        let latest = newest_for_notice(&home, now, || {
            fetches += 1;
            Ok(Some(version("0.8.0-beta.14")))
        });
        assert_eq!((fetches, latest), (1, Some(version("0.8.0-beta.14"))));
        let entry = read_cache(&cache_path(&home)).expect("cache written");
        assert_eq!(entry.checked_at_ms, now);
        assert_eq!(entry.latest_version.as_deref(), Some("0.8.0-beta.14"));

        let again = newest_for_notice(&home, now + 1, || panic!("checked a moment ago"));
        assert_eq!(again, Some(version("0.8.0-beta.14")));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn a_failed_check_is_cached_and_keeps_the_last_known_release() {
        let home = temp_home("failed");
        let now = 10 * DAY_MS;
        record_check(&home, now - 2 * DAY_MS, Ok(Some(version("0.8.0-beta.14")))).unwrap();

        let latest = newest_for_notice(&home, now, || anyhow::bail!("offline"));
        assert_eq!(latest, Some(version("0.8.0-beta.14")));
        let entry = read_cache(&cache_path(&home)).expect("failure is cached");
        assert_eq!(entry.checked_at_ms, now);

        let retried = newest_for_notice(&home, now + 1, || panic!("no retry within a day"));
        assert_eq!(retried, Some(version("0.8.0-beta.14")));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn a_first_failed_check_records_nothing_newer() {
        let home = temp_home("first-failure");
        let latest = newest_for_notice(&home, DAY_MS, || anyhow::bail!("offline"));
        assert_eq!(latest, None);
        let entry = read_cache(&cache_path(&home)).expect("failure is cached");
        assert_eq!((entry.checked_at_ms, entry.latest_version), (DAY_MS, None));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn freshness_expires_after_a_day_and_on_a_clock_moved_backwards() {
        let entry = CacheEntry {
            checked_at_ms: 5 * DAY_MS,
            latest_version: None,
        };
        assert!(entry.is_fresh(5 * DAY_MS));
        assert!(entry.is_fresh(6 * DAY_MS - 1));
        assert!(!entry.is_fresh(6 * DAY_MS));
        assert!(!entry.is_fresh(5 * DAY_MS - 1));
    }

    #[test]
    fn an_unreadable_cache_counts_as_stale() {
        let home = temp_home("corrupt");
        fs::create_dir_all(&home).unwrap();
        fs::write(cache_path(&home), b"not json").unwrap();
        let latest = newest_for_notice(&home, DAY_MS, || Ok(Some(version("0.8.0-beta.14"))));
        assert_eq!(latest, Some(version("0.8.0-beta.14")));
        let _ = fs::remove_dir_all(&home);
    }
}

//! Self-update from the app's GitHub releases.
//!
//! The app asks GitHub for its repository's releases and offers the newest one,
//! together with the notes of every release since the running version. When the
//! user accepts, it downloads the archive for this platform and checks it against
//! the release's `SHA256SUMS` and the digest GitHub records. It then extracts the
//! executable next to the running one and, on restart, swaps the two. Windows
//! allows renaming a running executable but not overwriting it, so the swap
//! renames the running file aside first. The next start removes what is left.
//!
//! The archive layout is a contract with every installed copy of the app:
//! `<bin>-<version>-<platform>.zip` (or `.tar.gz` on Linux) holding
//! `<bin>-<version>-<platform>/<bin>[.exe]`, next to a `SHA256SUMS` asset.
//! `scripts/package-release.sh` writes exactly that.
//!
//! This file is identical in every app. Copy a change to all of them.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The largest release archive the updater downloads.
const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
/// The largest executable the updater extracts from an archive.
const MAX_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;
/// The largest `SHA256SUMS` file the updater downloads.
const MAX_SUMS_BYTES: u64 = 64 * 1024;
/// How many releases one check reads. Older notes stay on the releases page.
const RELEASES_PER_CHECK: usize = 30;
const SUMS_ASSET: &str = "SHA256SUMS";
/// Debug builds read this variable as the running version, so a local build
/// can be updated to the current release to test the whole flow.
#[cfg(debug_assertions)]
const PRETEND_VERSION_VAR: &str = "APP_UPDATE_PRETEND_VERSION";
/// Names the swap leaves beside the executable: the staged update, then the
/// replaced executable. A replaced executable that is still running cannot be
/// deleted, so later updates fall back to numbered names.
const STAGED_SUFFIX: &str = "new";
const BACKUP_SUFFIXES: [&str; 4] = ["old", "old1", "old2", "old3"];

/// Which app is updating, and where its releases are.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppInfo {
    /// The GitHub account that owns the repository.
    pub owner: &'static str,
    /// The repository name.
    pub repo: &'static str,
    /// The executable name without `.exe`, which also prefixes each archive.
    pub bin: &'static str,
    /// The running version, normally `env!("CARGO_PKG_VERSION")`.
    pub version: &'static str,
}

impl AppInfo {
    /// The page listing every release, for when the updater cannot help.
    #[must_use]
    pub fn releases_page(&self) -> String {
        format!("https://github.com/{}/{}/releases", self.owner, self.repo)
    }
}

/// The release platform this build runs on, named as `package-release.sh`
/// names it, or `None` where no release is published.
#[must_use]
pub const fn platform() -> Option<&'static str> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Some("windows-x86_64")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("linux-x86_64")
    } else {
        None
    }
}

fn is_windows_platform(platform: &str) -> bool {
    platform.starts_with("windows-")
}

fn archive_name(bin: &str, version: Version, platform: &str) -> String {
    let extension = if is_windows_platform(platform) {
        "zip"
    } else {
        "tar.gz"
    };
    format!("{bin}-{version}-{platform}.{extension}")
}

fn archive_entry(bin: &str, version: Version, platform: &str) -> String {
    let extension = if is_windows_platform(platform) {
        ".exe"
    } else {
        ""
    };
    format!("{bin}-{version}-{platform}/{bin}{extension}")
}

/// A release version: exactly `major.minor.patch`, with an optional leading
/// `v`. Pre-release and build suffixes are rejected rather than guessed at.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Version {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.strip_prefix('v').unwrap_or(text);
        let mut numbers = [0_u64; 3];
        let mut parts = text.split('.');
        for number in &mut numbers {
            let part = parts.next()?;
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            *number = part.parse().ok()?;
        }
        if parts.next().is_some() {
            return None;
        }
        let [major, minor, patch] = numbers;
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// How often the app checks for a new release on its own.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum CheckPeriod {
    EveryStart,
    #[default]
    Daily,
    Weekly,
    Monthly,
    Never,
}

impl CheckPeriod {
    pub const ALL: [Self; 5] = [
        Self::EveryStart,
        Self::Daily,
        Self::Weekly,
        Self::Monthly,
        Self::Never,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::EveryStart => "Every time the app starts",
            Self::Daily => "Once a day",
            Self::Weekly => "Once a week",
            Self::Monthly => "Once a month",
            Self::Never => "Never",
        }
    }

    const fn interval_secs(self) -> Option<u64> {
        match self {
            Self::Daily => Some(24 * 60 * 60),
            Self::Weekly => Some(7 * 24 * 60 * 60),
            Self::Monthly => Some(30 * 24 * 60 * 60),
            Self::EveryStart | Self::Never => None,
        }
    }
}

/// What the app persists for the updater.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub period: CheckPeriod,
    /// When a check last succeeded, in seconds since the Unix epoch.
    pub last_check: Option<u64>,
    /// A version the user chose to skip. A newer release notifies again.
    pub skipped: Option<String>,
}

impl Settings {
    /// Whether an automatic check is due at `now`. `attempted` is when this
    /// session last tried one, successful or not, so a failed check is not
    /// retried on every frame.
    #[must_use]
    pub fn is_due(&self, now: u64, attempted: Option<u64>) -> bool {
        match self.period {
            CheckPeriod::Never => false,
            CheckPeriod::EveryStart => attempted.is_none(),
            period => {
                let Some(interval) = period.interval_secs() else {
                    return false;
                };
                match attempted.or(self.last_check) {
                    None => true,
                    // A clock moved backwards would otherwise postpone checks
                    // until it catches up again.
                    Some(reference) if reference > now => true,
                    Some(reference) => now - reference >= interval,
                }
            }
        }
    }
}

/// The current time in seconds since the Unix epoch.
#[must_use]
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// A short phrase for how long ago `then` was, such as "3 days ago".
#[must_use]
pub fn describe_age(then: u64, now: u64) -> String {
    let seconds = now.saturating_sub(then);
    let (count, unit) = match seconds {
        0..60 => return "just now".to_owned(),
        60..3_600 => (seconds / 60, "minute"),
        3_600..86_400 => (seconds / 3_600, "hour"),
        _ => (seconds / 86_400, "day"),
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural} ago")
}

/// One downloadable file attached to a release.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    /// The SHA-256 GitHub records for the upload, when it records one.
    pub digest: Option<[u8; 32]>,
}

/// A published release newer than the running version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Release {
    pub version: Version,
    pub tag: String,
    /// The publication date as `YYYY-MM-DD`, or empty when GitHub omits it.
    pub date: String,
    /// The release notes, in Markdown. See [`parse_notes`].
    pub notes: String,
    pub page_url: String,
    /// The archive for this platform.
    pub archive: Option<Asset>,
    pub sums: Option<Asset>,
}

impl Release {
    /// Whether this release carries everything an install needs.
    #[must_use]
    pub fn installable(&self) -> bool {
        self.archive.is_some() && self.sums.is_some()
    }
}

/// The releases newer than the running version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Update {
    latest: Release,
    earlier: Vec<Release>,
}

impl Update {
    #[must_use]
    pub fn latest(&self) -> &Release {
        &self.latest
    }

    /// Every newer release, newest first, starting with [`Update::latest`].
    pub fn releases(&self) -> impl Iterator<Item = &Release> {
        std::iter::once(&self.latest).chain(&self.earlier)
    }
}

/// One block of release notes, ready to lay out.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NoteBlock {
    Heading(String),
    Bullet(String),
    Paragraph(String),
}

/// Reads the small Markdown subset `CHANGELOG.md` uses: headings, bullets
/// whose text wraps onto indented lines, and paragraphs. Links keep their
/// text, and emphasis and code markers are dropped.
#[must_use]
pub fn parse_notes(markdown: &str) -> Vec<NoteBlock> {
    fn flush(blocks: &mut Vec<NoteBlock>, open: &mut Option<NoteBlock>) {
        if let Some(block) = open.take() {
            blocks.push(match block {
                NoteBlock::Heading(text) => NoteBlock::Heading(plain_text(&text)),
                NoteBlock::Bullet(text) => NoteBlock::Bullet(plain_text(&text)),
                NoteBlock::Paragraph(text) => NoteBlock::Paragraph(plain_text(&text)),
            });
        }
    }

    let mut blocks = Vec::new();
    let mut open = None;
    for line in markdown.lines() {
        let line = line.trim();
        if line.is_empty() {
            flush(&mut blocks, &mut open);
            continue;
        }
        let heading = line.trim_start_matches('#');
        if heading.len() < line.len() && (heading.is_empty() || heading.starts_with(' ')) {
            flush(&mut blocks, &mut open);
            open = Some(NoteBlock::Heading(heading.trim().to_owned()));
            flush(&mut blocks, &mut open);
            continue;
        }
        if let Some(item) = ["- ", "* ", "+ "]
            .iter()
            .find_map(|marker| line.strip_prefix(marker))
        {
            flush(&mut blocks, &mut open);
            open = Some(NoteBlock::Bullet(item.trim().to_owned()));
            continue;
        }
        match &mut open {
            Some(NoteBlock::Bullet(text) | NoteBlock::Paragraph(text)) => {
                text.push(' ');
                text.push_str(line);
            }
            _ => open = Some(NoteBlock::Paragraph(line.to_owned())),
        }
    }
    flush(&mut blocks, &mut open);
    blocks.retain(|block| match block {
        NoteBlock::Heading(text) | NoteBlock::Bullet(text) | NoteBlock::Paragraph(text) => {
            !text.is_empty()
        }
    });
    blocks
}

/// Turns `[text](url)` into `text` and drops `**`, `__`, and backticks.
fn plain_text(markdown: &str) -> String {
    let mut text = String::with_capacity(markdown.len());
    let mut rest = markdown;
    while let Some(start) = rest.find('[') {
        let after = &rest[start + 1..];
        let link = after.find("](").and_then(|middle| {
            let target = &after[middle + 2..];
            target.find(')').map(|end| (middle, middle + 2 + end + 1))
        });
        if let Some((label_end, link_end)) = link {
            text.push_str(&rest[..start]);
            text.push_str(&after[..label_end]);
            rest = &after[link_end..];
        } else {
            text.push_str(&rest[..=start]);
            rest = after;
        }
    }
    text.push_str(rest);
    text.replace("**", "").replace("__", "").replace('`', "")
}

/// Why an update could not be checked, downloaded, or installed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("file error: {0}")]
    Io(#[from] io::Error),
    #[error("GitHub returned {status}: {message}")]
    Status { status: u16, message: String },
    #[error("the running version {0:?} is not a release version")]
    UnknownVersion(&'static str),
    #[error("this release has no build for this platform")]
    NoBuild,
    #[error("a download link points outside this app's GitHub releases")]
    UnexpectedUrl,
    #[error("the download is larger than an update can be")]
    TooLarge,
    #[error("the download was {actual} bytes, but GitHub listed {expected}")]
    SizeMismatch { expected: u64, actual: u64 },
    #[error("SHA256SUMS does not list {0}")]
    NotInSums(String),
    #[error("the download does not match its published SHA-256 checksum")]
    ChecksumMismatch,
    #[error("the archive does not contain {0}")]
    MissingExecutable(String),
    #[error("the archive is damaged: {0}")]
    Archive(String),
    #[error("the downloaded file is not an executable for this platform")]
    NotExecutable,
    #[error("the downloaded update changed on disk after it was verified")]
    StagedChanged,
    #[error("the location of the running executable is unknown")]
    NoExecutable,
    #[error("the replaced executables beside the app are all still in use")]
    BackupsInUse,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    size: u64,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
}

#[derive(Deserialize)]
struct ApiError {
    message: String,
}

/// Keeps the published, non-prerelease releases newer than `current`, newest
/// first, and picks out the assets `platform` installs from.
fn select_update(
    app: &AppInfo,
    current: Version,
    platform: Option<&str>,
    releases: Vec<ApiRelease>,
) -> Option<Update> {
    let mut newer: Vec<Release> = releases
        .into_iter()
        .filter(|release| !release.draft && !release.prerelease)
        .filter_map(|release| {
            let version = Version::parse(&release.tag_name)?;
            (version > current).then_some((version, release))
        })
        .map(|(version, release)| {
            let find = |name: &str| {
                release
                    .assets
                    .iter()
                    .find(|asset| asset.name == name)
                    .map(|asset| Asset {
                        name: asset.name.clone(),
                        url: asset.browser_download_url.clone(),
                        size: asset.size,
                        digest: asset
                            .digest
                            .as_deref()
                            .and_then(|digest| digest.strip_prefix("sha256:"))
                            .and_then(parse_sha256),
                    })
            };
            let archive =
                platform.and_then(|platform| find(&archive_name(app.bin, version, platform)));
            Release {
                version,
                page_url: format!(
                    "https://github.com/{}/{}/releases/tag/{}",
                    app.owner, app.repo, release.tag_name
                ),
                date: release
                    .published_at
                    .as_deref()
                    .and_then(|date| date.get(..10))
                    .unwrap_or_default()
                    .to_owned(),
                notes: release.body.clone().unwrap_or_default(),
                archive,
                sums: find(SUMS_ASSET),
                tag: release.tag_name,
            }
        })
        .collect();
    newer.sort_by_key(|release| std::cmp::Reverse(release.version));
    newer.dedup_by_key(|release| release.version);
    let mut releases = newer.into_iter();
    let latest = releases.next()?;
    Some(Update {
        latest,
        earlier: releases.collect(),
    })
}

fn parse_sha256(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 || !hex.is_ascii() {
        return None;
    }
    let mut hash = [0_u8; 32];
    for (index, byte) in hash.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(hash)
}

/// Finds `name`'s hash in `sha256sum` output, in text or binary mode.
fn sums_entry(sums: &str, name: &str) -> Option<[u8; 32]> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.split_once(' ')?;
        let file = file.strip_prefix([' ', '*'])?;
        (file == name).then(|| parse_sha256(hash)).flatten()
    })
}

fn read_limited(reader: impl Read, limit: u64) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit {
        return Err(Error::TooLarge);
    }
    Ok(bytes)
}

/// Reads one regular file out of a release archive. Only the exact expected
/// path is read, and nothing is written to disk, so no other entry matters.
fn extract(archive: &[u8], zip: bool, entry: &str) -> Result<Vec<u8>, Error> {
    let missing = || Error::MissingExecutable(entry.to_owned());
    if zip {
        let mut archive = zip::ZipArchive::new(io::Cursor::new(archive))
            .map_err(|error| Error::Archive(error.to_string()))?;
        let file = match archive.by_name(entry) {
            Ok(file) => file,
            Err(zip::result::ZipError::FileNotFound) => return Err(missing()),
            Err(error) => return Err(Error::Archive(error.to_string())),
        };
        if !file.is_file() {
            return Err(missing());
        }
        return read_limited(file, MAX_EXECUTABLE_BYTES);
    }
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    let entries = archive
        .entries()
        .map_err(|error| Error::Archive(error.to_string()))?;
    for file in entries {
        let file = file.map_err(|error| Error::Archive(error.to_string()))?;
        if file.header().entry_type().is_file() && &*file.path_bytes() == entry.as_bytes() {
            return read_limited(file, MAX_EXECUTABLE_BYTES);
        }
    }
    Err(missing())
}

fn looks_executable(program: &[u8], platform: &str) -> bool {
    if is_windows_platform(platform) {
        program.starts_with(b"MZ")
    } else {
        program.starts_with(b"\x7fELF")
    }
}

/// `<executable>.<suffix>`, beside the executable.
fn sibling(executable: &Path, suffix: &str) -> PathBuf {
    let mut name = executable.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(suffix);
    executable.with_file_name(name)
}

/// A verified executable written beside the running one, waiting for
/// [`install`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Staged {
    pub version: Version,
    path: PathBuf,
    sha256: [u8; 32],
}

/// Talks to GitHub for one app.
pub struct Client {
    http: reqwest::blocking::Client,
    app: AppInfo,
    current: Version,
    releases_url: String,
    download_prefix: String,
}

impl Client {
    /// # Errors
    /// Returns an error when the running version does not parse or the HTTP
    /// client cannot be built.
    pub fn new(app: AppInfo) -> Result<Self, Error> {
        let current = running_version(&app).ok_or(Error::UnknownVersion(app.version))?;
        Self::with_urls(
            app,
            current,
            format!(
                "https://api.github.com/repos/{}/{}/releases?per_page={RELEASES_PER_CHECK}",
                app.owner, app.repo
            ),
            format!(
                "https://github.com/{}/{}/releases/download/",
                app.owner, app.repo
            ),
        )
    }

    fn with_urls(
        app: AppInfo,
        current: Version,
        releases_url: String,
        download_prefix: String,
    ) -> Result<Self, Error> {
        // reqwest's `rustls-no-provider` leaves the crypto provider to the
        // application. Installing one is idempotent; an error only means
        // another client in this process installed one first.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = reqwest::blocking::Client::builder()
            .user_agent(format!("{}/{}", app.bin, app.version))
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_mins(10))
            .build()?;
        Ok(Self {
            http,
            app,
            current,
            releases_url,
            download_prefix,
        })
    }

    #[must_use]
    pub fn current(&self) -> Version {
        self.current
    }

    /// Lists the releases newer than the running version.
    ///
    /// # Errors
    /// Returns an error for transport, status, or decoding failures.
    pub fn check(&self) -> Result<Option<Update>, Error> {
        tracing::info!(current = %self.current, "checking for app updates");
        let response = self
            .http
            .get(&self.releases_url)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .timeout(Duration::from_secs(30))
            .send()?;
        let releases: Vec<ApiRelease> = success(response)?.json()?;
        let update = select_update(&self.app, self.current, platform(), releases);
        tracing::info!(
            latest = update.as_ref().map(|update| update.latest.tag.as_str()),
            "app update check finished"
        );
        Ok(update)
    }

    /// Downloads `release`'s archive, verifies it, and writes its executable
    /// beside `executable`, ready for [`install`].
    ///
    /// # Errors
    /// Returns an error when the release has no build for this platform, a
    /// link points elsewhere, a size or checksum does not match, the archive
    /// lacks the executable, or the file cannot be written.
    pub fn download(
        &self,
        release: &Release,
        executable: &Path,
        mut progress: impl FnMut(u64, Option<u64>),
    ) -> Result<Staged, Error> {
        let platform = platform().ok_or(Error::NoBuild)?;
        let (Some(archive), Some(sums)) = (&release.archive, &release.sums) else {
            return Err(Error::NoBuild);
        };
        let prefix = format!("{}{}/", self.download_prefix, release.tag);
        if !archive.url.starts_with(&prefix) || !sums.url.starts_with(&prefix) {
            return Err(Error::UnexpectedUrl);
        }
        if archive.size > MAX_ARCHIVE_BYTES {
            return Err(Error::TooLarge);
        }
        tracing::info!(release = %release.tag, bytes = archive.size, "downloading app update");
        let sums = self.fetch(sums, MAX_SUMS_BYTES, |_, _| {})?;
        let expected = sums_entry(&String::from_utf8_lossy(&sums), &archive.name)
            .ok_or_else(|| Error::NotInSums(archive.name.clone()))?;
        let bytes = self.fetch(archive, MAX_ARCHIVE_BYTES, &mut progress)?;
        let received = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if received != archive.size {
            return Err(Error::SizeMismatch {
                expected: archive.size,
                actual: received,
            });
        }
        let actual: [u8; 32] = Sha256::digest(&bytes).into();
        if actual != expected || archive.digest.is_some_and(|digest| digest != actual) {
            return Err(Error::ChecksumMismatch);
        }
        let entry = archive_entry(self.app.bin, release.version, platform);
        let program = extract(&bytes, is_windows_platform(platform), &entry)?;
        if !looks_executable(&program, platform) {
            return Err(Error::NotExecutable);
        }
        let staged = stage(executable, &program, release.version)?;
        tracing::info!(release = %release.tag, path = %staged.path.display(), "app update staged");
        Ok(staged)
    }

    fn fetch(
        &self,
        asset: &Asset,
        limit: u64,
        mut progress: impl FnMut(u64, Option<u64>),
    ) -> Result<Vec<u8>, Error> {
        let response = self
            .http
            .get(&asset.url)
            .header(reqwest::header::ACCEPT, "application/octet-stream")
            .send()?;
        let mut response = success(response)?;
        let total = response.content_length().or(Some(asset.size));
        if total.is_some_and(|total| total > limit) {
            return Err(Error::TooLarge);
        }
        let mut bytes = Vec::new();
        let mut buffer = vec![0_u8; 64 * 1024];
        progress(0, total);
        loop {
            let read = response.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
            let received = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
            if received > limit {
                return Err(Error::TooLarge);
            }
            progress(received, total);
        }
        Ok(bytes)
    }
}

fn running_version(app: &AppInfo) -> Option<Version> {
    #[cfg(debug_assertions)]
    if let Some(version) = std::env::var(PRETEND_VERSION_VAR)
        .ok()
        .as_deref()
        .and_then(Version::parse)
    {
        tracing::warn!(%version, "pretending to run another version");
        return Some(version);
    }
    Version::parse(app.version)
}

fn success(response: reqwest::blocking::Response) -> Result<reqwest::blocking::Response, Error> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let message = response
        .json::<ApiError>()
        .map(|error| error.message)
        .unwrap_or_default();
    let message: String = message.chars().take(200).collect();
    Err(Error::Status {
        status: status.as_u16(),
        message: if message.is_empty() {
            status
                .canonical_reason()
                .unwrap_or("no reason given")
                .to_owned()
        } else {
            message
        },
    })
}

fn stage(executable: &Path, program: &[u8], version: Version) -> Result<Staged, Error> {
    let path = sibling(executable, STAGED_SUFFIX);
    // Opened for writing so `sync_all` can flush: Windows refuses to flush a
    // read-only handle.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)?;
    file.write_all(program)?;
    file.sync_all()?;
    drop(file);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(Staged {
        version,
        path,
        sha256: Sha256::digest(program).into(),
    })
}

/// Renames, retrying briefly: antivirus scanners often hold a fresh
/// executable open for a moment after it is written.
fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
    let mut attempt = 0;
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error) if attempt >= 4 || error.kind() == io::ErrorKind::NotFound => {
                return Err(error);
            }
            Err(_) => {
                attempt += 1;
                std::thread::sleep(Duration::from_millis(250));
            }
        }
    }
}

/// Picks a free name for the executable being replaced. A replaced
/// executable that is still running, such as a second copy of the app,
/// cannot be deleted yet, so later updates move on to the next name.
fn free_backup(executable: &Path) -> Result<PathBuf, Error> {
    for suffix in BACKUP_SUFFIXES {
        let backup = sibling(executable, suffix);
        match fs::remove_file(&backup) {
            Ok(()) => return Ok(backup),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(backup),
            Err(error) => {
                tracing::debug!(path = %backup.display(), %error, "replaced executable still in use");
            }
        }
    }
    Err(Error::BackupsInUse)
}

/// Swaps the staged executable into `executable`'s place. The running file
/// is renamed aside first and put back if the swap fails.
///
/// # Errors
/// Returns an error when the staged file changed since it was verified, or
/// either rename fails.
pub fn install(staged: &Staged, executable: &Path) -> Result<(), Error> {
    let program = fs::read(&staged.path)?;
    if <[u8; 32]>::from(Sha256::digest(&program)) != staged.sha256 {
        return Err(Error::StagedChanged);
    }
    let backup = free_backup(executable)?;
    rename_with_retry(executable, &backup)?;
    if let Err(error) = rename_with_retry(&staged.path, executable) {
        if let Err(restore) = fs::rename(&backup, executable) {
            tracing::error!(%restore, backup = %backup.display(), "could not put the replaced executable back");
        }
        return Err(error.into());
    }
    tracing::info!(version = %staged.version, path = %executable.display(), "app update installed");
    Ok(())
}

/// Deletes what earlier updates left beside `executable`. A file that is
/// still in use stays until a later start.
pub fn remove_leftovers(executable: &Path) {
    for suffix in std::iter::once(STAGED_SUFFIX).chain(BACKUP_SUFFIXES) {
        let path = sibling(executable, suffix);
        match fs::remove_file(&path) {
            Ok(()) => tracing::info!(path = %path.display(), "removed update leftover"),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::debug!(path = %path.display(), %error, "update leftover still in use");
            }
        }
    }
}

static RESTART: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Asks [`restart_if_requested`] to start `executable` once the app exits.
pub fn request_restart(executable: &Path) {
    if let Ok(mut restart) = RESTART.lock() {
        *restart = Some(executable.to_owned());
    }
}

/// Starts the executable [`request_restart`] named. Call this after the GUI
/// event loop returns, so the app's own state is saved first.
///
/// # Errors
/// Returns an error when the new process cannot be started.
pub fn restart_if_requested() -> io::Result<()> {
    let executable = RESTART.lock().ok().and_then(|mut restart| restart.take());
    if let Some(executable) = executable {
        tracing::info!(path = %executable.display(), "restarting into the updated app");
        std::process::Command::new(executable).spawn()?;
    }
    Ok(())
}

enum Message {
    Checked(Result<Option<Box<Update>>, String>),
    Progress(u64, Option<u64>),
    Downloaded(Result<Staged, String>),
}

/// Runs checks and downloads off the UI thread and holds what they found.
/// Call [`Updater::poll`] once per frame.
pub struct Updater {
    app: AppInfo,
    client: Option<Arc<Client>>,
    executable: Option<PathBuf>,
    repaint: Arc<dyn Fn() + Send + Sync>,
    sender: mpsc::Sender<Message>,
    receiver: mpsc::Receiver<Message>,
    /// When this session last started a check.
    attempted: Option<u64>,
    checking: bool,
    checked: bool,
    update: Option<Update>,
    error: Option<String>,
    progress: Option<(u64, Option<u64>)>,
    staged: Option<Staged>,
    dismissed: bool,
}

impl Updater {
    /// Records where the running executable is, before any update can move
    /// it, and removes what an earlier update left beside it. `repaint` wakes
    /// the UI when background work reports back.
    pub fn new(app: AppInfo, repaint: impl Fn() + Send + Sync + 'static) -> Self {
        let executable = std::env::current_exe()
            .inspect_err(|error| tracing::warn!(%error, "running executable not found"))
            .ok();
        if let Some(executable) = &executable {
            remove_leftovers(executable);
        }
        let (sender, receiver) = mpsc::channel();
        Self {
            app,
            client: None,
            executable,
            repaint: Arc::new(repaint),
            sender,
            receiver,
            attempted: None,
            checking: false,
            checked: false,
            update: None,
            error: None,
            progress: None,
            staged: None,
            dismissed: false,
        }
    }

    #[must_use]
    pub fn app(&self) -> AppInfo {
        self.app
    }

    /// The running version, or `None` when it does not parse.
    #[must_use]
    pub fn current(&self) -> Option<Version> {
        running_version(&self.app)
    }

    #[must_use]
    pub fn is_checking(&self) -> bool {
        self.checking
    }

    /// Whether a check finished this session and found nothing newer.
    #[must_use]
    pub fn is_up_to_date(&self) -> bool {
        self.checked && self.update.is_none()
    }

    #[must_use]
    pub fn update(&self) -> Option<&Update> {
        self.update.as_ref()
    }

    /// The last check or download failure.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Bytes received and expected while a download runs.
    #[must_use]
    pub fn progress(&self) -> Option<(u64, Option<u64>)> {
        self.progress
    }

    /// The version waiting for a restart, once downloaded and verified.
    #[must_use]
    pub fn staged(&self) -> Option<Version> {
        self.staged.as_ref().map(|staged| staged.version)
    }

    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.checking || self.progress.is_some()
    }

    /// Whether the latest release can be downloaded and installed here.
    #[must_use]
    pub fn can_install(&self) -> bool {
        self.executable.is_some()
            && platform().is_some()
            && self
                .update
                .as_ref()
                .is_some_and(|update| update.latest.installable())
    }

    /// The release to point out, unless the user skipped it or set it aside
    /// for this session.
    #[must_use]
    pub fn notification(&self, settings: &Settings) -> Option<&Release> {
        let latest = self.update.as_ref()?.latest();
        let skipped = settings.skipped.as_deref() == Some(latest.version.to_string().as_str());
        (!self.dismissed && !skipped).then_some(latest)
    }

    /// Hides the notification until the app restarts.
    pub fn dismiss(&mut self) {
        self.dismissed = true;
    }

    /// Hides the notification for the latest release until a newer one.
    pub fn skip(&mut self, settings: &mut Settings) {
        if let Some(update) = &self.update {
            settings.skipped = Some(update.latest.version.to_string());
        }
    }

    /// Takes in background results and starts an automatic check when one
    /// is due.
    pub fn poll(&mut self, settings: &mut Settings) {
        while let Ok(message) = self.receiver.try_recv() {
            match message {
                Message::Checked(result) => {
                    self.checking = false;
                    match result {
                        Ok(update) => {
                            settings.last_check = Some(now());
                            self.checked = true;
                            self.error = None;
                            if self.staged.as_ref().map(|staged| staged.version)
                                != update.as_ref().map(|update| update.latest.version)
                            {
                                self.staged = None;
                            }
                            self.update = update.map(|update| *update);
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
                Message::Progress(received, total) => {
                    if self.progress.is_some() {
                        self.progress = Some((received, total));
                    }
                }
                Message::Downloaded(result) => {
                    self.progress = None;
                    match result {
                        Ok(staged) => {
                            self.error = None;
                            self.staged = Some(staged);
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
            }
        }
        let now = now();
        if !self.is_busy() && settings.is_due(now, self.attempted) {
            self.check_now();
        }
    }

    fn client(&mut self) -> Result<Arc<Client>, String> {
        if let Some(client) = &self.client {
            return Ok(Arc::clone(client));
        }
        let client = Arc::new(Client::new(self.app).map_err(|error| error.to_string())?);
        self.client = Some(Arc::clone(&client));
        Ok(client)
    }

    /// Checks for a newer release now, unless a check or download is running.
    pub fn check_now(&mut self) {
        if self.is_busy() {
            return;
        }
        self.attempted = Some(now());
        let client = match self.client() {
            Ok(client) => client,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        self.checking = true;
        let sender = self.sender.clone();
        let repaint = Arc::clone(&self.repaint);
        self.spawn("update-check", move || {
            let result = client
                .check()
                .map(|update| update.map(Box::new))
                .map_err(|error| {
                    tracing::warn!(%error, "app update check failed");
                    error.to_string()
                });
            let _ = sender.send(Message::Checked(result));
            repaint();
        });
    }

    /// Downloads and verifies the latest release in the background.
    pub fn download(&mut self) {
        if self.is_busy() || !self.can_install() {
            return;
        }
        let (Some(update), Some(executable)) = (&self.update, &self.executable) else {
            return;
        };
        let release = update.latest.clone();
        let executable = executable.clone();
        let client = match self.client() {
            Ok(client) => client,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        self.error = None;
        self.progress = Some((0, release.archive.as_ref().map(|archive| archive.size)));
        let sender = self.sender.clone();
        let repaint = Arc::clone(&self.repaint);
        self.spawn("update-download", move || {
            let mut last_report = 0;
            let result = client
                .download(&release, &executable, |received, total| {
                    // A report per 256 KiB keeps the bar moving without
                    // waking the UI for every network read.
                    if received == 0 || received - last_report >= 256 * 1024 {
                        last_report = received;
                        let _ = sender.send(Message::Progress(received, total));
                        repaint();
                    }
                })
                .map_err(|error| {
                    tracing::warn!(%error, "app update download failed");
                    error.to_string()
                });
            let _ = sender.send(Message::Downloaded(result));
            repaint();
        });
    }

    /// Swaps the downloaded executable in and asks for a restart once the
    /// app exits. The caller then closes the window; until it does, the
    /// app's own executable path already holds the new version, so it must
    /// not start any copy of itself.
    ///
    /// # Errors
    /// Returns a message when nothing is staged or the swap fails.
    pub fn install_and_restart(&mut self) -> Result<(), String> {
        let (Some(staged), Some(executable)) = (&self.staged, &self.executable) else {
            return Err("No downloaded update is waiting to be installed.".to_owned());
        };
        if let Err(error) = install(staged, executable) {
            let message = format!("The update could not be installed: {error}");
            self.error = Some(message.clone());
            return Err(message);
        }
        request_restart(executable);
        self.staged = None;
        Ok(())
    }

    fn spawn(&mut self, name: &str, work: impl FnOnce() + Send + 'static) {
        if let Err(error) = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(work)
        {
            self.checking = false;
            self.progress = None;
            self.error = Some(format!("Could not start the update thread: {error}"));
        }
    }

    #[cfg(test)]
    fn with_client(app: AppInfo, client: Client, executable: PathBuf) -> Self {
        let mut updater = Self::new(app, || {});
        updater.client = Some(Arc::new(client));
        updater.executable = Some(executable);
        updater
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    const APP: AppInfo = AppInfo {
        owner: "owner",
        repo: "repo",
        bin: "tool",
        version: "1.2.3",
    };

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn versions_parse_strictly_and_order_numerically() {
        assert_eq!(
            Version::parse("v1.2.3"),
            Some(Version {
                major: 1,
                minor: 2,
                patch: 3
            })
        );
        assert_eq!(version("1.2.3"), version("v1.2.3"));
        assert!(version("1.10.0") > version("1.9.9"));
        assert!(version("2.0.0") > version("1.99.99"));
        for bad in [
            "",
            "1.2",
            "1.2.3.4",
            "1.2.3-rc.1",
            "1.2.x",
            "+1.2.3",
            "1..3",
            "vv1.2.3",
            " 1.2.3",
        ] {
            assert_eq!(Version::parse(bad), None, "{bad:?}");
        }
        assert_eq!(version("v0.3.10").to_string(), "0.3.10");
    }

    #[test]
    fn check_period_decides_when_a_check_is_due() {
        let day = 24 * 60 * 60;
        let now = 100 * day;
        let settings = |period, last_check| Settings {
            period,
            last_check,
            skipped: None,
        };
        assert!(!settings(CheckPeriod::Never, None).is_due(now, None));
        assert!(settings(CheckPeriod::EveryStart, Some(now)).is_due(now, None));
        assert!(!settings(CheckPeriod::EveryStart, None).is_due(now, Some(now)));
        assert!(settings(CheckPeriod::Daily, None).is_due(now, None));
        assert!(!settings(CheckPeriod::Daily, Some(now - day + 1)).is_due(now, None));
        assert!(settings(CheckPeriod::Daily, Some(now - day)).is_due(now, None));
        assert!(!settings(CheckPeriod::Weekly, Some(now - 6 * day)).is_due(now, None));
        assert!(settings(CheckPeriod::Weekly, Some(now - 7 * day)).is_due(now, None));
        assert!(!settings(CheckPeriod::Monthly, Some(now - 29 * day)).is_due(now, None));
        assert!(settings(CheckPeriod::Monthly, Some(now - 30 * day)).is_due(now, None));
        // A failed check this session waits a full period, like a success.
        assert!(!settings(CheckPeriod::Daily, None).is_due(now, Some(now - 60)));
        assert!(settings(CheckPeriod::Daily, Some(now - 2 * day)).is_due(now, Some(now - day)));
        // A clock that moved backwards does not postpone the next check.
        assert!(settings(CheckPeriod::Weekly, Some(now + day)).is_due(now, None));
    }

    #[test]
    fn settings_load_from_older_or_partial_state() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, Settings::default());
        assert_eq!(settings.period, CheckPeriod::Daily);
        let settings: Settings =
            serde_json::from_str(r#"{"period":"Weekly","skipped":"1.4.0"}"#).unwrap();
        assert_eq!(settings.period, CheckPeriod::Weekly);
        assert_eq!(settings.skipped.as_deref(), Some("1.4.0"));
    }

    #[test]
    fn ages_read_naturally() {
        assert_eq!(describe_age(100, 100), "just now");
        assert_eq!(describe_age(100, 50), "just now");
        assert_eq!(describe_age(0, 60), "1 minute ago");
        assert_eq!(describe_age(0, 7_200), "2 hours ago");
        assert_eq!(describe_age(0, 3 * 86_400 + 5), "3 days ago");
    }

    #[test]
    fn notes_join_wrapped_bullets_and_strip_markup() {
        let notes = "### Fixes\n\n\
            - Stop offering DLSS updates for games on DLSS 1, such as Battlefield V. A DLSS 2 or later DLL\n  \
              turns DLSS off in those games.\n\
            - See the [releases page](https://example.com/releases) for `older` **notes**.\n\
            * Second marker\n\n\
            A closing paragraph\nthat wraps.\n\n\
            ##\n\
            - [unclosed link\n";
        assert_eq!(
            parse_notes(notes),
            vec![
                NoteBlock::Heading("Fixes".into()),
                NoteBlock::Bullet(
                    "Stop offering DLSS updates for games on DLSS 1, such as Battlefield V. \
                     A DLSS 2 or later DLL turns DLSS off in those games."
                        .into()
                ),
                NoteBlock::Bullet("See the releases page for older notes.".into()),
                NoteBlock::Bullet("Second marker".into()),
                NoteBlock::Paragraph("A closing paragraph that wraps.".into()),
                NoteBlock::Bullet("[unclosed link".into()),
            ]
        );
        assert_eq!(
            parse_notes("#hashtag"),
            vec![NoteBlock::Paragraph("#hashtag".into())]
        );
        assert!(parse_notes("\n\n").is_empty());
    }

    fn api_release(tag: &str, assets: &[&str]) -> ApiRelease {
        ApiRelease {
            tag_name: tag.to_owned(),
            draft: false,
            prerelease: false,
            body: Some(format!("- Notes for {tag}")),
            published_at: Some("2026-10-01T12:00:00Z".to_owned()),
            assets: assets
                .iter()
                .map(|name| ApiAsset {
                    name: (*name).to_owned(),
                    size: 10,
                    browser_download_url: format!("https://example.com/{tag}/{name}"),
                    digest: Some(format!("sha256:{}", "ab".repeat(32))),
                })
                .collect(),
        }
    }

    #[test]
    fn selection_keeps_newer_published_releases_newest_first() {
        let mut draft = api_release("v9.0.0", &[]);
        draft.draft = true;
        let mut prerelease = api_release("v8.0.0", &[]);
        prerelease.prerelease = true;
        let releases = vec![
            api_release("v1.2.4", &["tool-1.2.4-windows-x86_64.zip", "SHA256SUMS"]),
            draft,
            prerelease,
            api_release("v1.2.3", &[]),
            api_release("v1.0.0", &[]),
            api_release("nightly", &[]),
            api_release("v1.3.0-rc.1", &[]),
            api_release(
                "v1.10.0",
                &[
                    "tool-1.10.0-windows-x86_64.zip",
                    "tool-1.10.0-linux-x86_64.tar.gz",
                ],
            ),
        ];
        let update =
            select_update(&APP, version("1.2.3"), Some("windows-x86_64"), releases).unwrap();
        let tags: Vec<_> = update
            .releases()
            .map(|release| release.tag.as_str())
            .collect();
        assert_eq!(tags, ["v1.10.0", "v1.2.4"]);
        let latest = update.latest();
        assert_eq!(latest.date, "2026-10-01");
        assert_eq!(latest.notes, "- Notes for v1.10.0");
        assert_eq!(
            latest.page_url,
            "https://github.com/owner/repo/releases/tag/v1.10.0"
        );
        let archive = latest.archive.as_ref().unwrap();
        assert_eq!(archive.name, "tool-1.10.0-windows-x86_64.zip");
        assert_eq!(archive.digest, Some([0xab; 32]));
        // No SHA256SUMS, so the newest release can be read but not installed.
        assert!(!latest.installable());
        assert!(update.releases().nth(1).unwrap().installable());

        let linux = select_update(
            &APP,
            version("1.2.3"),
            Some("linux-x86_64"),
            vec![api_release(
                "v1.10.0",
                &["tool-1.10.0-linux-x86_64.tar.gz", "SHA256SUMS"],
            )],
        )
        .unwrap();
        assert!(linux.latest().installable());
        let unknown = select_update(
            &APP,
            version("1.2.3"),
            None,
            vec![api_release(
                "v1.10.0",
                &["tool-1.10.0-linux-x86_64.tar.gz", "SHA256SUMS"],
            )],
        )
        .unwrap();
        assert!(!unknown.latest().installable());
        assert!(
            select_update(
                &APP,
                version("1.10.0"),
                None,
                vec![api_release("v1.10.0", &[])]
            )
            .is_none()
        );
    }

    #[test]
    fn sums_match_the_exact_file_name() {
        let hash = "0f".repeat(32);
        let sums = format!(
            "{hash}  other.zip\n{hash}  tool.zip.sig\n{} *tool.zip\n",
            "1a".repeat(32)
        );
        assert_eq!(sums_entry(&sums, "tool.zip"), Some([0x1a; 32]));
        assert_eq!(sums_entry(&sums, "other.zip"), Some([0x0f; 32]));
        assert_eq!(sums_entry(&sums, "missing.zip"), None);
        assert_eq!(
            sums_entry(&format!("{}  tool.zip", "zz".repeat(32)), "tool.zip"),
            None
        );
        assert_eq!(
            sums_entry(&format!("{}  tool.zip", "ab".repeat(31)), "tool.zip"),
            None
        );
        assert_eq!(sums_entry("", "tool.zip"), None);
    }

    fn zip_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            if let Some(directory) = name.strip_suffix('/') {
                writer
                    .add_directory(directory, zip::write::SimpleFileOptions::default())
                    .unwrap();
            } else {
                writer
                    .start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(bytes).unwrap();
            }
        }
        writer.finish().unwrap().into_inner()
    }

    fn tar_gz_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        for (name, bytes) in entries {
            let mut header = tar::Header::new_gnu();
            if name.ends_with('/') {
                header.set_entry_type(tar::EntryType::Directory);
                header.set_size(0);
            } else {
                header.set_size(bytes.len() as u64);
            }
            header.set_mode(0o755);
            builder.append_data(&mut header, name, *bytes).unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn extraction_reads_only_the_expected_file() {
        let entry = "tool-1.3.0-windows-x86_64/tool.exe";
        for zip in [true, false] {
            let build = if zip { zip_archive } else { tar_gz_archive };
            let archive = build(&[
                ("tool-1.3.0-windows-x86_64/README.md", b"readme"),
                (entry, b"MZ program"),
            ]);
            assert_eq!(extract(&archive, zip, entry).unwrap(), b"MZ program");

            let elsewhere = build(&[("other/tool.exe", b"MZ program"), ("tool.exe", b"MZ")]);
            assert!(matches!(
                extract(&elsewhere, zip, entry),
                Err(Error::MissingExecutable(_))
            ));

            let directory = build(&[("tool-1.3.0-windows-x86_64/tool.exe/", b"")]);
            assert!(matches!(
                extract(&directory, zip, entry),
                Err(Error::MissingExecutable(_))
            ));

            assert!(matches!(
                extract(b"not an archive", zip, entry),
                Err(Error::Archive(_))
            ));
        }
    }

    #[test]
    fn only_executables_for_the_platform_pass() {
        assert!(looks_executable(b"MZ\x90\x00", "windows-x86_64"));
        assert!(!looks_executable(b"\x7fELF", "windows-x86_64"));
        assert!(looks_executable(b"\x7fELF\x02", "linux-x86_64"));
        assert!(!looks_executable(b"#!/bin/sh", "linux-x86_64"));
        assert!(!looks_executable(b"", "linux-x86_64"));
    }

    #[test]
    fn install_swaps_the_executable_and_keeps_the_old_one_aside() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("tool.exe");
        fs::write(&executable, b"old program").unwrap();
        let staged = stage(&executable, b"new program", version("1.3.0")).unwrap();
        assert_eq!(staged.path, directory.path().join("tool.exe.new"));
        install(&staged, &executable).unwrap();
        assert_eq!(fs::read(&executable).unwrap(), b"new program");
        assert_eq!(
            fs::read(directory.path().join("tool.exe.old")).unwrap(),
            b"old program"
        );
        assert!(!staged.path.exists());

        // The next update replaces the earlier leftover.
        let staged = stage(&executable, b"newer program", version("1.4.0")).unwrap();
        install(&staged, &executable).unwrap();
        assert_eq!(fs::read(&executable).unwrap(), b"newer program");
        assert_eq!(
            fs::read(directory.path().join("tool.exe.old")).unwrap(),
            b"new program"
        );
    }

    #[test]
    fn install_refuses_a_staged_file_that_changed_and_leaves_the_app_alone() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("tool.exe");
        fs::write(&executable, b"old program").unwrap();
        let staged = stage(&executable, b"new program", version("1.3.0")).unwrap();
        fs::write(&staged.path, b"swapped program").unwrap();
        assert!(matches!(
            install(&staged, &executable),
            Err(Error::StagedChanged)
        ));
        assert_eq!(fs::read(&executable).unwrap(), b"old program");

        fs::remove_file(&staged.path).unwrap();
        assert!(matches!(install(&staged, &executable), Err(Error::Io(_))));
        assert_eq!(fs::read(&executable).unwrap(), b"old program");
        assert!(!directory.path().join("tool.exe.old").exists());
    }

    #[test]
    fn leftovers_are_removed_by_exact_name_only() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("tool.exe");
        for name in [
            "tool.exe",
            "tool.exe.new",
            "tool.exe.old",
            "tool.exe.old2",
            "tool.exe.older",
            "tool.exe.bak",
            "other.exe.old",
        ] {
            fs::write(directory.path().join(name), b"x").unwrap();
        }
        remove_leftovers(&executable);
        let mut left: Vec<_> = fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                "other.exe.old",
                "tool.exe",
                "tool.exe.bak",
                "tool.exe.older"
            ]
        );
    }

    type Route = (String, u16, Vec<u8>);

    /// Serves fixed responses over plain HTTP, one per connection. The routes
    /// are built from the server's address, which only exists once it listens.
    fn serve(routes: impl FnOnce(&str) -> Vec<Route>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let routes = routes(&base);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                if reader.read_line(&mut request).is_err() {
                    continue;
                }
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
                        break;
                    }
                }
                let path = request.split_whitespace().nth(1).unwrap_or_default();
                let (status, body) = routes.iter().find(|(route, _, _)| route == path).map_or(
                    (404, b"{\"message\":\"Not Found\"}".to_vec()),
                    |(_, status, body)| (*status, body.clone()),
                );
                let head = format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        base
    }

    fn hex(hash: &[u8; 32]) -> String {
        use std::fmt::Write as _;
        hash.iter().fold(String::new(), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
    }

    /// What the fake GitHub serves for release v1.3.0. [`Served::new`]
    /// builds a correct release; each test breaks one part of it.
    struct Served {
        archive: Vec<u8>,
        sums: String,
        digest: Option<[u8; 32]>,
        /// Where the release says the archive is, relative to the server.
        archive_path: String,
        listed_size: usize,
    }

    impl Served {
        fn new(platform: &str) -> Self {
            Self::with_program(platform, &Self::program(platform))
        }

        fn program(platform: &str) -> Vec<u8> {
            let magic: &[u8] = if is_windows_platform(platform) {
                b"MZ"
            } else {
                b"\x7fELF"
            };
            [magic, b" new program"].concat()
        }

        fn with_program(platform: &str, program: &[u8]) -> Self {
            let new = version("1.3.0");
            let entry = archive_entry(APP.bin, new, platform);
            let archive = if is_windows_platform(platform) {
                zip_archive(&[(&entry, program)])
            } else {
                tar_gz_archive(&[(&entry, program)])
            };
            let hash: [u8; 32] = Sha256::digest(&archive).into();
            let name = archive_name(APP.bin, new, platform);
            Self {
                sums: format!("{}  other.zip\n{}  {name}\n", "00".repeat(32), hex(&hash)),
                digest: Some(hash),
                archive_path: format!("/download/v1.3.0/{name}"),
                listed_size: archive.len(),
                archive,
            }
        }

        /// Starts the server and returns a client for the running version
        /// 1.2.3 that talks to it.
        fn start(self, platform: &str) -> Client {
            let name = archive_name(APP.bin, version("1.3.0"), platform);
            let base = serve(|base| {
                let digest = self.digest.map_or(String::new(), |digest| {
                    format!(r#","digest":"sha256:{}""#, hex(&digest))
                });
                let releases = format!(
                    r#"[{{"tag_name":"v1.3.0","body":"- New things","published_at":"2026-10-20T00:00:00Z","assets":[
                        {{"name":"{name}","size":{size},"browser_download_url":"{base}{path}"{digest}}},
                        {{"name":"SHA256SUMS","size":{sums_size},"browser_download_url":"{base}/download/v1.3.0/SHA256SUMS"}}]}},
                       {{"tag_name":"v1.2.3","assets":[]}}]"#,
                    size = self.listed_size,
                    path = self.archive_path,
                    sums_size = self.sums.len(),
                );
                vec![
                    ("/releases".to_owned(), 200, releases.into_bytes()),
                    (self.archive_path.clone(), 200, self.archive),
                    (
                        "/download/v1.3.0/SHA256SUMS".to_owned(),
                        200,
                        self.sums.into_bytes(),
                    ),
                ]
            });
            Client::with_urls(
                APP,
                version("1.2.3"),
                format!("{base}/releases"),
                format!("{base}/download/"),
            )
            .unwrap()
        }
    }

    /// A temporary folder holding the "running" executable.
    fn running_app(platform: &str) -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let name = if is_windows_platform(platform) {
            "tool.exe"
        } else {
            "tool"
        };
        let executable = directory.path().join(name);
        fs::write(&executable, b"old program").unwrap();
        (directory, executable)
    }

    /// Checks, then downloads the latest release beside a fresh executable.
    fn check_and_download(
        served: Served,
        platform: &str,
    ) -> Result<(Staged, PathBuf, tempfile::TempDir), Error> {
        let client = served.start(platform);
        let release = client.check().unwrap().unwrap().latest().clone();
        let (directory, executable) = running_app(platform);
        let staged = client.download(&release, &executable, |_, _| {})?;
        Ok((staged, executable, directory))
    }

    #[test]
    fn download_verifies_and_stages_the_new_executable() {
        let Some(platform) = platform() else { return };
        let served = Served::new(platform);
        let size = served.archive.len() as u64;
        let client = served.start(platform);
        let update = client.check().unwrap().unwrap();
        assert_eq!(update.releases().count(), 1);
        let release = update.latest();
        assert_eq!(release.notes, "- New things");
        assert!(release.installable());

        let (_directory, executable) = running_app(platform);
        let mut reports = Vec::new();
        let staged = client
            .download(release, &executable, |received, total| {
                reports.push((received, total));
            })
            .unwrap();
        assert_eq!(staged.version, version("1.3.0"));
        assert_eq!(fs::read(&staged.path).unwrap(), Served::program(platform));
        assert_eq!(fs::read(&executable).unwrap(), b"old program");
        assert_eq!(reports.first(), Some(&(0, Some(size))));
        assert_eq!(reports.last(), Some(&(size, Some(size))));

        install(&staged, &executable).unwrap();
        assert_eq!(fs::read(&executable).unwrap(), Served::program(platform));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&executable).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o755);
        }
    }

    #[test]
    fn download_accepts_a_release_without_a_github_digest() {
        let Some(platform) = platform() else { return };
        let mut served = Served::new(platform);
        served.digest = None;
        assert!(check_and_download(served, platform).is_ok());
    }

    #[test]
    fn download_rejects_a_checksum_that_does_not_match() {
        let Some(platform) = platform() else { return };
        let mut served = Served::new(platform);
        let name = archive_name(APP.bin, version("1.3.0"), platform);
        served.sums = format!("{}  {name}\n", "11".repeat(32));
        assert!(matches!(
            check_and_download(served, platform),
            Err(Error::ChecksumMismatch)
        ));

        let mut served = Served::new(platform);
        served.digest = Some([0x22; 32]);
        assert!(matches!(
            check_and_download(served, platform),
            Err(Error::ChecksumMismatch)
        ));

        let mut served = Served::new(platform);
        served.sums = format!("{}  {name}.sig\n", "11".repeat(32));
        assert!(matches!(
            check_and_download(served, platform),
            Err(Error::NotInSums(_))
        ));
    }

    #[test]
    fn download_rejects_links_outside_the_release() {
        let Some(platform) = platform() else { return };
        let name = archive_name(APP.bin, version("1.3.0"), platform);
        for path in [
            format!("/download/v1.2.3/{name}"),
            format!("/elsewhere/v1.3.0/{name}"),
        ] {
            let mut served = Served::new(platform);
            served.archive_path = path;
            assert!(matches!(
                check_and_download(served, platform),
                Err(Error::UnexpectedUrl)
            ));
        }
    }

    #[test]
    fn download_rejects_a_size_that_differs_from_the_listing() {
        let Some(platform) = platform() else { return };
        let mut served = Served::new(platform);
        served.listed_size += 1;
        assert!(matches!(
            check_and_download(served, platform),
            Err(Error::SizeMismatch { .. })
        ));
    }

    #[test]
    fn download_rejects_an_archive_without_a_real_executable() {
        let Some(platform) = platform() else { return };
        let served = Served::with_program(platform, b"#!/bin/sh\necho not a program");
        let result = check_and_download(served, platform);
        assert!(matches!(result, Err(Error::NotExecutable)));
    }

    #[test]
    fn check_reports_the_message_github_returns() {
        let base = serve(|_| {
            vec![(
                "/releases".to_owned(),
                403,
                br#"{"message":"API rate limit exceeded"}"#.to_vec(),
            )]
        });
        let client = Client::with_urls(
            APP,
            version("1.2.3"),
            format!("{base}/releases"),
            base.clone(),
        )
        .unwrap();
        let error = client.check().unwrap_err();
        assert_eq!(
            error.to_string(),
            "GitHub returned 403: API rate limit exceeded"
        );
    }

    fn poll_until(updater: &mut Updater, settings: &mut Settings, done: impl Fn(&Updater) -> bool) {
        for _ in 0..500 {
            updater.poll(settings);
            if done(updater) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the updater did not finish: {:?}", updater.error());
    }

    #[test]
    fn updater_checks_notifies_skips_and_installs() {
        let Some(platform) = platform() else { return };
        let client = Served::new(platform).start(platform);
        let (_directory, executable) = running_app(platform);
        let mut updater = Updater::with_client(APP, client, executable.clone());
        let mut settings = Settings {
            period: CheckPeriod::EveryStart,
            ..Settings::default()
        };

        // The first poll starts the check, because one is due.
        poll_until(&mut updater, &mut settings, |updater| {
            updater.update().is_some()
        });
        assert!(settings.last_check.is_some());
        assert_eq!(updater.notification(&settings).unwrap().tag, "v1.3.0");
        assert!(updater.can_install());

        updater.skip(&mut settings);
        assert_eq!(settings.skipped.as_deref(), Some("1.3.0"));
        assert!(updater.notification(&settings).is_none());
        settings.skipped = Some("1.2.9".to_owned());
        assert!(updater.notification(&settings).is_some());
        updater.dismiss();
        assert!(updater.notification(&settings).is_none());

        // Every start means once per start, not on every frame.
        updater.poll(&mut settings);
        assert!(!updater.is_checking());

        assert!(updater.install_and_restart().is_err());
        updater.download();
        assert!(updater.progress().is_some());
        poll_until(&mut updater, &mut settings, |updater| {
            updater.staged().is_some()
        });
        assert_eq!(updater.error(), None);
        updater.install_and_restart().unwrap();
        assert_eq!(fs::read(&executable).unwrap(), Served::program(platform));
        let restart = RESTART.lock().unwrap().take();
        assert_eq!(restart, Some(executable));
        assert_eq!(updater.staged(), None);
    }

    #[test]
    fn updater_with_checks_turned_off_stays_quiet() {
        let mut updater = Updater::new(APP, || {});
        let mut settings = Settings {
            period: CheckPeriod::Never,
            ..Settings::default()
        };
        updater.poll(&mut settings);
        assert!(!updater.is_checking());
        assert!(updater.client.is_none());
        assert_eq!(settings.last_check, None);
    }
}

use std::fs;
use std::path::{Path, PathBuf};

use jiff::{Timestamp, Zoned};
use serde::{Deserialize, Serialize};

use crate::error::StatsError;
use crate::plan::{Options, Plan};

/// Schema version written into every stats file, so a future format change
/// can migrate or skip old files deliberately.
pub const STATS_SCHEMA_VERSION: u32 = 1;

/// Outcome of one execution run, persisted as JSON to the stats directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecResult {
    pub started_at: Timestamp,
    pub finished_at: Timestamp,
    #[serde(default)]
    pub groups: Vec<GroupResult>,
    pub schema_version: u32,
    pub duration_ms: i64,
    pub total_selected: usize,
    pub total_bytes: u64,
    pub error_count: usize,
    pub dry_run: bool,
}

/// Per-group outcome inside an [`ExecResult`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupResult {
    pub app: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<PathError>,
    pub bytes: u64,
    pub paths_attempted: usize,
    pub paths_failed: usize,
}

/// One failed path inside a [`GroupResult`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathError {
    pub path: String,
    pub error: String,
}

impl ExecResult {
    /// Starts a new result for the given plan, timestamped now.
    #[must_use]
    pub fn begin(plan: &Plan, opts: Options) -> Self {
        Self {
            started_at: Timestamp::now(),
            finished_at: Timestamp::now(),
            groups: Vec::new(),
            schema_version: STATS_SCHEMA_VERSION,
            duration_ms: 0,
            total_selected: plan.selected,
            total_bytes: plan.total_bytes,
            error_count: 0,
            dry_run: opts.dry_run,
        }
    }

    /// Stamps the finish time and duration.
    pub fn finish(&mut self) {
        self.finished_at = Timestamp::now();
        let elapsed = self.finished_at.as_millisecond() - self.started_at.as_millisecond();
        self.duration_ms = elapsed.max(0);
    }

    /// The timestamp representing this run: finished time when present,
    /// otherwise the start time.
    #[must_use]
    pub fn run_timestamp(&self) -> Timestamp {
        if self.finished_at == Timestamp::default() {
            self.started_at
        } else {
            self.finished_at
        }
    }
}

/// Writes `result` as pretty JSON into `dir`, creating it if needed. The
/// filename uses local time (`YYYYMMDD-HHMMSS-mmm.json`) so a directory
/// listing sorts chronologically. Returns the written path.
///
/// # Errors
///
/// Returns an error when the directory cannot be created or the file cannot
/// be written or encoded.
pub fn write_stats(dir: &Path, result: &mut ExecResult) -> Result<PathBuf, StatsError> {
    fs::create_dir_all(dir).map_err(|source| StatsError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    result.schema_version = STATS_SCHEMA_VERSION;

    let local: Zoned = result.finished_at.to_zoned(jiff::tz::TimeZone::system());
    let millis = result.finished_at.as_millisecond().rem_euclid(1000);
    let name = format!("{}-{millis:03}.json", local.strftime("%Y%m%d-%H%M%S"));
    let path = dir.join(name);

    let data = serde_json::to_vec_pretty(result)?;
    fs::write(&path, data).map_err(|source| StatsError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

/// One stats file successfully loaded from the stats directory. The source
/// path is kept so a run can be deleted from history.
#[derive(Debug, Clone)]
pub struct StoredRun {
    pub path: PathBuf,
    pub result: ExecResult,
}

/// Loads every readable stats file from `dir` (a missing directory yields no
/// results). Returns the runs plus the number of files that could not be
/// read or parsed.
///
/// # Errors
///
/// Returns an error when an existing stats directory cannot be listed.
pub fn load_stats(dir: &Path) -> Result<(Vec<StoredRun>, usize), StatsError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok((Vec::new(), 0)),
        Err(source) => {
            return Err(StatsError::Io {
                path: dir.to_path_buf(),
                source,
            });
        }
    };

    let mut results = Vec::new();
    let mut skipped = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() || path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let Ok(data) = fs::read(&path) else {
            skipped += 1;
            continue;
        };
        match serde_json::from_slice::<ExecResult>(&data) {
            Ok(result) => results.push(StoredRun { path, result }),
            Err(_) => skipped += 1,
        }
    }
    Ok((results, skipped))
}

/// Deletes every stats JSON file in `dir`, readable or not. Returns the number
/// of files removed; a missing directory is a no-op.
///
/// # Errors
///
/// Returns an error when the directory cannot be listed or a file cannot be
/// removed.
pub fn clear_stats(dir: &Path) -> Result<usize, StatsError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(source) => {
            return Err(StatsError::Io {
                path: dir.to_path_buf(),
                source,
            });
        }
    };

    let mut removed = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() || path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        fs::remove_file(&path).map_err(|source| StatsError::Io {
            path: path.clone(),
            source,
        })?;
        removed += 1;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
  "started_at": "2026-07-01T15:04:05.123456789Z",
  "finished_at": "2026-07-01T15:04:07.987654321Z",
  "groups": [
    {
      "app": "Chrome",
      "label": "all profiles cache",
      "bytes": 123456,
      "paths_attempted": 3,
      "paths_failed": 0
    },
    {
      "app": "Discord",
      "label": "cache + logs",
      "errors": [
        {
          "path": "C:\\Users\\me\\AppData\\Roaming\\discord\\Cache",
          "error": "SHFileOperationW failed, code=32"
        }
      ],
      "bytes": 789,
      "paths_attempted": 1,
      "paths_failed": 1
    }
  ],
  "schema_version": 1,
  "duration_ms": 2864,
  "total_selected": 2,
  "total_bytes": 124245,
  "error_count": 1,
  "dry_run": false
}"#;

    #[test]
    fn parses_stats_files() {
        let result: ExecResult = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(result.schema_version, 1);
        assert_eq!(result.groups.len(), 2);
        assert!(result.groups[0].errors.is_empty());
        assert_eq!(result.groups[1].errors.len(), 1);
        assert_eq!(result.duration_ms, 2864);
        assert_eq!(result.total_bytes, 124_245);
        assert!(!result.dry_run);
    }

    #[test]
    fn serialization_omits_empty_errors_and_keeps_field_names() {
        let result: ExecResult = serde_json::from_str(FIXTURE).unwrap();
        let json = serde_json::to_string_pretty(&result).unwrap();
        for field in [
            "started_at",
            "finished_at",
            "groups",
            "schema_version",
            "duration_ms",
            "total_selected",
            "total_bytes",
            "error_count",
            "dry_run",
            "paths_attempted",
            "paths_failed",
        ] {
            assert!(json.contains(&format!("\"{field}\"")), "missing {field}");
        }
        // Clean groups don't serialize an `errors` key.
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value["groups"][0].get("errors").is_none());
        assert!(value["groups"][1].get("errors").is_some());
        // Round-trip.
        let reparsed: ExecResult = serde_json::from_str(&json).unwrap();
        assert_eq!(reparsed.groups.len(), result.groups.len());
        assert_eq!(reparsed.started_at, result.started_at);
    }

    #[test]
    fn write_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let stats_dir = dir.path().join("stats");

        let mut result = ExecResult::begin(&Plan::default(), Options { dry_run: false });
        result.finish();
        let path = write_stats(&stats_dir, &mut result).unwrap();
        assert!(path.exists());

        let (loaded, skipped) = load_stats(&stats_dir).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(skipped, 0);
        assert_eq!(loaded[0].result.schema_version, STATS_SCHEMA_VERSION);
        assert_eq!(loaded[0].path, path);
    }

    #[test]
    fn clear_removes_stats_files_and_tolerates_missing_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.json"), FIXTURE).unwrap();
        fs::write(dir.path().join("broken.json"), "{not json").unwrap();
        fs::write(dir.path().join("kept.txt"), "nope").unwrap();

        assert_eq!(clear_stats(dir.path()).unwrap(), 2);
        let (loaded, skipped) = load_stats(dir.path()).unwrap();
        assert!(loaded.is_empty());
        assert_eq!(skipped, 0);
        assert!(dir.path().join("kept.txt").exists());

        assert_eq!(clear_stats(&dir.path().join("does-not-exist")).unwrap(), 0);
    }

    #[test]
    fn load_counts_invalid_and_tolerates_missing_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.json"), FIXTURE).unwrap();
        fs::write(dir.path().join("broken.json"), "{not json").unwrap();
        fs::write(dir.path().join("ignored.txt"), "nope").unwrap();

        let (loaded, skipped) = load_stats(dir.path()).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(skipped, 1);

        let (loaded, skipped) = load_stats(&dir.path().join("does-not-exist")).unwrap();
        assert!(loaded.is_empty());
        assert_eq!(skipped, 0);
    }

    #[test]
    fn stats_filename_shape() {
        let dir = tempfile::tempdir().unwrap();
        let mut result = ExecResult::begin(&Plan::default(), Options::default());
        result.finish();
        let path = write_stats(dir.path(), &mut result).unwrap();
        let name = path.file_name().unwrap().to_str().unwrap();
        // YYYYMMDD-HHMMSS-mmm.json
        assert_eq!(name.len(), "20260701-180405-123.json".len());
        assert!(
            Path::new(name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        );
    }
}

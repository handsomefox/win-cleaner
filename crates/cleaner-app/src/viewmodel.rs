//! Toolkit-agnostic category mapping, filtering, sorting, and text summaries.
//! Everything here is unit-tested without a GUI.

use cleaner_core::{ExecResult, Plan, human_bytes};
use jiff::{Timestamp, Zoned};

use crate::strings::UiText;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SortMode {
    #[default]
    Name,
    SizeDesc,
    SizeAsc,
}

/// One collapsible category section on the selection screen.
pub(crate) struct CategoryView {
    pub name: String,
    pub apps: Vec<AppView>,
    pub bytes: u64,
}

/// One collapsible app section inside a category. `indices` point into
/// `plan.groups` so the UI mutates selection state without holding
/// references.
pub(crate) struct AppView {
    pub app: String,
    pub indices: Vec<usize>,
    pub bytes: u64,
}

/// Maps an app name to its category display name.
pub(crate) fn category_name<'t>(texts: &'t UiText, app_name: &str) -> &'t str {
    match app_name.to_lowercase().as_str() {
        "chrome" | "edge" | "firefox" | "brave" | "opera" | "vivaldi" => texts.category_browsers,
        "discord" | "slack" | "signal" | "teams (classic)" | "teams (new)" | "telegram"
        | "whatsapp" | "zoom" => texts.category_chat,
        "cargo" | "go modules" | "npm" | "yarn" | "pnpm" | "pip" | "gradle" | "maven" | "nuget"
        | "jetbrains" | "vscode" | "visual studio" | "unity" => texts.category_development,
        "battle.net"
        | "battlefield 2042"
        | "ea/origin"
        | "epic games launcher"
        | "gog galaxy"
        | "steam"
        | "vortex"
        | "ubisoft connect"
        | "rockstar games launcher"
        | "osu! (lazer)" => texts.category_gaming,
        "spotify" | "obs studio" => texts.category_media,
        "amd"
        | "crash dumps"
        | "nvidia"
        | "razer synapse"
        | "windows"
        | "windows error reporting" => texts.category_system,
        "adobe" | "blender" | "figma" => texts.category_creative,
        "empty folders" => texts.category_empty_folders,
        _ => texts.category_other,
    }
}

/// Applies the search filter and sort mode, returning app groups whose
/// visible items are sorted largest-first.
fn filtered_app_groups(plan: &Plan, filter: &str, sort: SortMode) -> Vec<AppView> {
    let filter = filter.trim().to_lowercase();
    let mut apps: Vec<AppView> = plan
        .by_app()
        .into_iter()
        .filter_map(|group| {
            let mut visible: Vec<usize> = group
                .indices
                .into_iter()
                .filter(|&index| {
                    if filter.is_empty() {
                        return true;
                    }
                    let g = &plan.groups[index];
                    format!("{} {}", g.app, g.label)
                        .to_lowercase()
                        .contains(&filter)
                })
                .collect();
            if visible.is_empty() {
                return None;
            }
            visible.sort_by(|&a, &b| plan.groups[b].bytes.cmp(&plan.groups[a].bytes));
            Some(AppView {
                app: group.app,
                indices: visible,
                bytes: group.bytes,
            })
        })
        .collect();

    match sort {
        SortMode::SizeDesc => apps.sort_by_key(|app| std::cmp::Reverse(app.bytes)),
        SortMode::SizeAsc => apps.sort_by_key(|app| app.bytes),
        SortMode::Name => {}
    }
    apps
}

/// Groups the filtered apps into category sections, sorted per the mode
/// (alphabetical for [`SortMode::Name`]).
pub(crate) fn categorized_app_groups(
    texts: &UiText,
    plan: &Plan,
    filter: &str,
    sort: SortMode,
) -> Vec<CategoryView> {
    let mut order: Vec<String> = Vec::new();
    let mut categories: Vec<CategoryView> = Vec::new();
    for app in filtered_app_groups(plan, filter, sort) {
        let name = category_name(texts, &app.app).to_owned();
        let position = if let Some(position) = order.iter().position(|existing| *existing == name) {
            position
        } else {
            order.push(name.clone());
            categories.push(CategoryView {
                name,
                apps: Vec::new(),
                bytes: 0,
            });
            categories.len() - 1
        };
        categories[position].bytes += app.bytes;
        categories[position].apps.push(app);
    }

    match sort {
        SortMode::SizeDesc => categories.sort_by_key(|category| std::cmp::Reverse(category.bytes)),
        SortMode::SizeAsc => categories.sort_by_key(|category| category.bytes),
        SortMode::Name => categories.sort_by(|a, b| a.name.cmp(&b.name)),
    }
    categories
}

/// Header summary for the selection screen: selection chip text plus the
/// savings chip text (empty when nothing is selected).
pub(crate) fn cache_selection_summary(texts: &UiText, plan: &mut Plan) -> (String, String) {
    plan.recompute_totals();
    let selection = texts.items_selected(plan.selected);
    let savings = if plan.total_bytes > 0 {
        texts.estimated_savings(plan.total_bytes)
    } else {
        String::new()
    };
    (selection, savings)
}

/// Headline plus summary line for the results screen.
pub(crate) fn cleanup_result_summary(
    texts: &UiText,
    result: &ExecResult,
    had_error: bool,
) -> (String, String) {
    let headline = if had_error || result.error_count > 0 {
        texts.result_cleanup_with_errors
    } else {
        texts.result_cleanup_complete
    };
    (headline.to_owned(), texts.cache_result_summary(result))
}

/// Joins non-empty parts with the `  |  ` separator used across summaries.
pub(crate) fn summary_line(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("  |  ")
}

pub(crate) fn format_duration(ms: i64) -> String {
    if ms <= 0 {
        return "--".to_owned();
    }
    // Round to the nearest second (ms is positive here).
    let seconds = (ms + 500) / 1000;
    let (hours, minutes, secs) = (seconds / 3600, (seconds % 3600) / 60, seconds % 60);
    match (hours, minutes) {
        (0, 0) => format!("{secs}s"),
        (0, _) => format!("{minutes}m{secs}s"),
        _ => format!("{hours}h{minutes}m{secs}s"),
    }
}

pub(crate) fn format_timestamp(timestamp: Timestamp) -> String {
    to_local(timestamp).strftime("%b %-d, %Y %H:%M").to_string()
}

pub(crate) fn format_timestamp_long(timestamp: Timestamp) -> String {
    to_local(timestamp)
        .strftime("%b %-d, %Y %H:%M:%S")
        .to_string()
}

fn to_local(timestamp: Timestamp) -> Zoned {
    timestamp.to_zoned(jiff::tz::TimeZone::system())
}

/// The plain-text detail pane for one history run.
pub(crate) fn run_details_text(texts: &UiText, result: &ExecResult) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} {}",
        texts.run_label_run,
        format_timestamp_long(result.run_timestamp())
    );
    let _ = writeln!(
        out,
        "{} {}",
        texts.run_label_duration,
        format_duration(result.duration_ms)
    );
    let _ = writeln!(out, "{} {}", texts.run_label_items, result.total_selected);
    let _ = writeln!(
        out,
        "{} {}",
        texts.run_label_freed,
        human_bytes(result.total_bytes)
    );
    let _ = writeln!(out, "{} {}", texts.run_label_errors, result.error_count);
    out.push('\n');

    if result.groups.is_empty() {
        let _ = writeln!(out, "{}", texts.result_no_group_details);
        return out;
    }
    for group in &result.groups {
        let status = if group.paths_failed > 0 {
            texts.failed_count(group.paths_failed)
        } else if group.paths_attempted == 0 {
            texts.result_status_skipped.to_owned()
        } else {
            texts.result_status_ok.to_owned()
        };
        let _ = writeln!(
            out,
            "{} - {}  [{}  {}]",
            group.app,
            group.label,
            human_bytes(group.bytes),
            status
        );
        for error in &group.errors {
            let _ = writeln!(out, "  ! {}: {}", error.path, error.error);
        }
    }
    out
}

/// The aggregate summary above the history list (runs, totals, 7/30 days).
pub(crate) fn stats_summary_text(
    texts: &UiText,
    results: &[ExecResult],
    skipped: usize,
    now: Timestamp,
) -> String {
    const DAY: i64 = 24 * 60 * 60;
    let cutoff7 = now - jiff::SignedDuration::from_secs(7 * DAY);
    let cutoff30 = now - jiff::SignedDuration::from_secs(30 * DAY);

    let mut total_all = 0u64;
    let mut total7 = 0u64;
    let mut total30 = 0u64;
    for result in results {
        total_all += result.total_bytes;
        let ts = result.run_timestamp();
        if ts > cutoff7 {
            total7 += result.total_bytes;
        }
        if ts > cutoff30 {
            total30 += result.total_bytes;
        }
    }

    let runs = if skipped > 0 {
        format!(
            "{} {}  ({skipped} {})",
            texts.stats_runs_label,
            results.len(),
            texts.stats_unreadable
        )
    } else {
        format!("{} {}", texts.stats_runs_label, results.len())
    };
    [
        runs,
        format!(
            "{} {}",
            texts.stats_total_freed_label,
            human_bytes(total_all)
        ),
        format!("{} {}", texts.stats_last7_label, human_bytes(total7)),
        format!("{} {}", texts.stats_last30_label, human_bytes(total30)),
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strings::ENGLISH;
    use cleaner_core::{Group, GroupResult, Options, PathError};

    fn group(app: &str, label: &str, bytes: u64, on: bool) -> Group {
        Group {
            app: app.to_owned(),
            label: label.to_owned(),
            paths: Vec::new(),
            errs: Vec::new(),
            bytes,
            on,
        }
    }

    fn sample_plan() -> Plan {
        let mut plan = Plan {
            groups: vec![
                group("Chrome", "all profiles cache", 500, true),
                group("npm", "package cache", 2000, true),
                group("Notion", "cache", 100, false),
                group("Empty folders", r"AppData\Local", 0, false),
                group("Windows", "Temp folder", 9000, true),
            ],
            ..Plan::default()
        };
        plan.recompute_totals();
        plan
    }

    #[test]
    fn category_mapping_places_apps_in_expected_sections() {
        assert_eq!(category_name(&ENGLISH, "Chrome"), "Browsers");
        assert_eq!(category_name(&ENGLISH, "Teams (new)"), "Chat");
        assert_eq!(category_name(&ENGLISH, "Go modules"), "Development");
        assert_eq!(category_name(&ENGLISH, "osu! (lazer)"), "Gaming");
        assert_eq!(category_name(&ENGLISH, "OBS Studio"), "Media");
        assert_eq!(category_name(&ENGLISH, "Windows Error Reporting"), "System");
        assert_eq!(category_name(&ENGLISH, "Blender"), "Creative");
        assert_eq!(category_name(&ENGLISH, "Empty folders"), "Empty folders");
        assert_eq!(category_name(&ENGLISH, "Vortex"), "Gaming");
        assert_eq!(category_name(&ENGLISH, "Razer Synapse"), "System");
        for app in ["Notion", "qBittorrent"] {
            assert_eq!(category_name(&ENGLISH, app), "Other", "{app}");
        }
    }

    #[test]
    fn categorized_groups_sort_by_name_and_by_size() {
        let plan = sample_plan();
        let by_name = categorized_app_groups(&ENGLISH, &plan, "", SortMode::Name);
        let names: Vec<&str> = by_name.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "Browsers",
                "Development",
                "Empty folders",
                "Other",
                "System"
            ]
        );

        let by_size = categorized_app_groups(&ENGLISH, &plan, "", SortMode::SizeDesc);
        assert_eq!(by_size[0].name, "System");
        assert_eq!(by_size[0].bytes, 9000);
    }

    #[test]
    fn filter_matches_app_and_label() {
        let plan = sample_plan();
        let filtered = categorized_app_groups(&ENGLISH, &plan, "package", SortMode::Name);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].apps[0].app, "npm");

        let filtered = categorized_app_groups(&ENGLISH, &plan, "CHROME", SortMode::Name);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Browsers");

        assert!(categorized_app_groups(&ENGLISH, &plan, "zzz", SortMode::Name).is_empty());
    }

    #[test]
    fn selection_summary_recomputes_totals() {
        let mut plan = sample_plan();
        plan.groups[0].on = false;
        let (selection, savings) = cache_selection_summary(&ENGLISH, &mut plan);
        assert_eq!(selection, "2 items selected");
        assert_eq!(savings, "Est. savings: 10.74 KB");
        assert_eq!(plan.selected, 2);

        for g in &mut plan.groups {
            g.on = false;
        }
        let (selection, savings) = cache_selection_summary(&ENGLISH, &mut plan);
        assert_eq!(selection, "0 items selected");
        assert!(savings.is_empty());
    }

    fn sample_result(errors: usize) -> ExecResult {
        let mut result = ExecResult::begin(&Plan::default(), Options::default());
        result.total_selected = 3;
        result.total_bytes = 1024;
        result.duration_ms = 2500;
        result.error_count = errors;
        result.groups = vec![
            GroupResult {
                app: "Chrome".into(),
                label: "cache".into(),
                errors: Vec::new(),
                bytes: 1024,
                paths_attempted: 2,
                paths_failed: 0,
            },
            GroupResult {
                app: "npm".into(),
                label: "cache".into(),
                errors: (0..errors)
                    .map(|i| PathError {
                        path: format!("/p{i}"),
                        error: "locked".into(),
                    })
                    .collect(),
                bytes: 0,
                paths_attempted: errors,
                paths_failed: errors,
            },
        ];
        result
    }

    #[test]
    fn result_summary_reports_errors() {
        let (headline, summary) = cleanup_result_summary(&ENGLISH, &sample_result(0), false);
        assert_eq!(headline, "Cleanup complete");
        assert_eq!(
            summary,
            "3 items cleaned  |  est. 1.00 KB freed  |  3s  |  0 errors"
        );

        let (headline, _) = cleanup_result_summary(&ENGLISH, &sample_result(2), false);
        assert_eq!(headline, "Cleanup finished with errors");
    }

    #[test]
    fn run_details_lists_groups_and_errors() {
        let details = run_details_text(&ENGLISH, &sample_result(1));
        assert!(details.contains("Items cleaned: 3"));
        assert!(details.contains("Chrome - cache  [1.00 KB  OK]"));
        assert!(details.contains("npm - cache  [0 B  1 failed]"));
        assert!(details.contains("! /p0: locked"));
    }

    #[test]
    fn stats_summary_windows_by_age() {
        let now = Timestamp::now();
        let day = jiff::SignedDuration::from_secs(24 * 60 * 60);

        let mut recent = sample_result(0);
        recent.finished_at = now - day;
        recent.total_bytes = 100;
        let mut old = sample_result(0);
        old.finished_at = now - day * 10;
        old.total_bytes = 1000;
        let mut ancient = sample_result(0);
        ancient.finished_at = now - day * 40;
        ancient.total_bytes = 10000;

        let text = stats_summary_text(&ENGLISH, &[recent, old, ancient], 1, now);
        assert!(text.contains("Runs: 3  (1 unreadable)"));
        assert!(text.contains("Total freed: 10.84 KB"));
        assert!(text.contains("Last 7 days: 100 B"));
        assert!(text.contains("Last 30 days: 1.07 KB"));
    }

    #[test]
    fn duration_formatting() {
        assert_eq!(format_duration(0), "--");
        assert_eq!(format_duration(-5), "--");
        assert_eq!(format_duration(1400), "1s");
        assert_eq!(format_duration(65_000), "1m5s");
        assert_eq!(format_duration(3_660_000), "1h1m0s");
    }
}

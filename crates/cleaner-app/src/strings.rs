//! All user-facing text lives in one struct so the UI stays translatable and
//! the text remains easy to review.

use cleaner_core::{ExecResult, Plan, ProgressUpdate, human_bytes};

use crate::viewmodel::format_duration;

pub(crate) struct UiText {
    pub app_title: &'static str,

    pub task_cache_scanning: &'static str,
    pub task_cache_deleting: &'static str,
    pub task_history: &'static str,
    pub task_history_failed: &'static str,

    pub action_cancel: &'static str,
    pub action_clean_up: &'static str,
    pub action_preview: &'static str,
    pub action_select_all: &'static str,
    pub action_select_non_empty: &'static str,
    pub action_deselect_all: &'static str,
    pub action_expand_all: &'static str,
    pub action_collapse_all: &'static str,
    pub action_rescan: &'static str,
    pub action_clean_again: &'static str,
    pub action_history: &'static str,
    pub action_done: &'static str,
    pub action_back: &'static str,
    pub action_refresh: &'static str,

    pub cache_scan_status: &'static str,
    pub cache_scan_card_title: &'static str,
    pub cache_scan_card_subtitle: &'static str,
    pub cache_sort_name: &'static str,
    pub cache_sort_largest: &'static str,
    pub cache_sort_smallest: &'static str,
    pub cache_search_hint: &'static str,
    pub category_browsers: &'static str,
    pub category_chat: &'static str,
    pub category_development: &'static str,
    pub category_gaming: &'static str,
    pub category_media: &'static str,
    pub category_system: &'static str,
    pub category_creative: &'static str,
    pub category_empty_folders: &'static str,
    pub category_other: &'static str,
    pub toggle_preview_only: &'static str,
    pub no_matching_cache_targets: &'static str,
    pub cache_delete_card_title: &'static str,
    pub cache_delete_card_subtitle: &'static str,

    pub status_preparing: &'static str,
    pub not_found: &'static str,

    pub progress_moving_title: &'static str,
    pub stat_selected: &'static str,
    pub stat_scope: &'static str,
    pub stat_destination: &'static str,
    pub recycle_bin: &'static str,
    pub unit_items: &'static str,

    pub dialog_nothing_selected_title: &'static str,
    pub dialog_select_cache_group: &'static str,
    pub dialog_confirm_cache_title: &'static str,
    pub dialog_preview_title: &'static str,
    pub dialog_preview_empty: &'static str,
    pub dialog_cleanup_errors: &'static str,
    pub dialog_close: &'static str,
    pub dialog_cleanup_details_title: &'static str,

    pub result_no_group_details: &'static str,
    pub result_error_details: &'static str,
    pub result_details: &'static str,
    pub result_status_ok: &'static str,
    pub result_status_skipped: &'static str,
    pub result_no_matching_paths: &'static str,
    pub result_scan_issues: &'static str,
    pub result_cleanup_complete: &'static str,
    pub result_cleanup_with_errors: &'static str,
    pub freed_prefix: &'static str,

    pub history_no_runs: &'static str,
    pub history_previous_runs_title: &'static str,
    pub history_previous_runs_subtitle: &'static str,

    pub run_label_run: &'static str,
    pub run_label_duration: &'static str,
    pub run_label_items: &'static str,
    pub run_label_freed: &'static str,
    pub run_label_errors: &'static str,
    pub stats_runs_label: &'static str,
    pub stats_unreadable: &'static str,
    pub stats_total_freed_label: &'static str,
    pub stats_last7_label: &'static str,
    pub stats_last30_label: &'static str,

    pub menu_cache_cleanup: &'static str,
    pub menu_history: &'static str,
    pub menu_about: &'static str,
    pub menu_quit: &'static str,
    pub about_version_label: &'static str,
    pub about_fonts: &'static str,
    pub about_repo_label: &'static str,
    pub about_repo_url: &'static str,
    pub about_open_log: &'static str,

    pub unsupported_platform: &'static str,
}

pub(crate) const ENGLISH: UiText = UiText {
    app_title: "win-cleaner",

    task_cache_scanning: "Scanning cache locations…",
    task_cache_deleting: "Moving selected cache files to the Recycle Bin",
    task_history: "Cleanup history",
    task_history_failed: "Cleanup history unavailable",

    action_cancel: "Cancel",
    action_clean_up: "Clean Up",
    action_preview: "Preview",
    action_select_all: "Select All",
    action_select_non_empty: "Select Non-Empty",
    action_deselect_all: "Deselect All",
    action_expand_all: "Expand All",
    action_collapse_all: "Collapse All",
    action_rescan: "Rescan",
    action_clean_again: "Clean Again",
    action_history: "View History",
    action_done: "Done",
    action_back: "Back",
    action_refresh: "Refresh",

    cache_scan_status: "Scanning cache locations…",
    cache_scan_card_title: "Scanning Cache",
    cache_scan_card_subtitle: "Looking for cache locations to clean.",
    cache_sort_name: "Name",
    cache_sort_largest: "Largest first",
    cache_sort_smallest: "Smallest first",
    cache_search_hint: "Search apps and cleanup items…",
    category_browsers: "Browsers",
    category_chat: "Chat",
    category_development: "Development",
    category_gaming: "Gaming",
    category_media: "Media",
    category_system: "System",
    category_creative: "Creative",
    category_empty_folders: "Empty folders",
    category_other: "Other",
    toggle_preview_only: "Preview only",
    no_matching_cache_targets: "No matching cleanup targets.",
    cache_delete_card_title: "Cleaning Up",
    cache_delete_card_subtitle: "Moving files to the Recycle Bin.",

    status_preparing: "Preparing…",
    not_found: "Not found",

    progress_moving_title: "Moving selected items",
    stat_selected: "Selected",
    stat_scope: "Scope",
    stat_destination: "Destination",
    recycle_bin: "Recycle Bin",
    unit_items: "items",

    dialog_nothing_selected_title: "Nothing Selected",
    dialog_select_cache_group: "Select at least one item to clean.",
    dialog_confirm_cache_title: "Confirm Cleanup",
    dialog_preview_title: "Preview — what would be cleaned",
    dialog_preview_empty: "Nothing selected.",
    dialog_cleanup_errors: "Cleanup Errors",
    dialog_close: "Close",
    dialog_cleanup_details_title: "Cleanup Target Details",

    result_no_group_details: "No item details recorded.",
    result_error_details: "View error details",
    result_details: "Details",
    result_status_ok: "OK",
    result_status_skipped: "Skipped",
    result_no_matching_paths: "No matching paths",
    result_scan_issues: "Scan issues:",
    result_cleanup_complete: "Cleanup complete",
    result_cleanup_with_errors: "Cleanup finished with errors",
    freed_prefix: "Freed: ",

    history_no_runs: "No cleanup history yet.",
    history_previous_runs_title: "Previous Cleanup Runs",
    history_previous_runs_subtitle: "Unreadable stats files are counted and skipped.",

    run_label_run: "Run:",
    run_label_duration: "Duration:",
    run_label_items: "Items cleaned:",
    run_label_freed: "Est. freed:",
    run_label_errors: "Errors:",
    stats_runs_label: "Runs:",
    stats_unreadable: "unreadable",
    stats_total_freed_label: "Total freed:",
    stats_last7_label: "Last 7 days:",
    stats_last30_label: "Last 30 days:",

    menu_cache_cleanup: "Cache Cleanup",
    menu_history: "History",
    menu_about: "About win-cleaner",
    menu_quit: "Quit",
    about_version_label: "Version",
    about_fonts: "Inter — SIL Open Font License 1.1",
    about_repo_label: "Project page",
    about_repo_url: "https://github.com/handsomefox/win-cleaner",
    about_open_log: "Open log folder",

    unsupported_platform: "win-cleaner only supports Windows. Scanning and cleaning are disabled on this platform.",
};

pub(crate) fn plural<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 { one } else { many }
}

// Formatting helpers take `&self` even when the English text is inlined, so a
// future locale only has to swap the receiver.
#[allow(clippy::unused_self, reason = "receiver reserved for localization")]
impl UiText {
    pub(crate) fn items_count(&self, n: usize) -> String {
        format!("{n} {}", plural(n, "item", "items"))
    }

    pub(crate) fn apps_count(&self, n: usize) -> String {
        format!("{n} {}", plural(n, "app", "apps"))
    }

    pub(crate) fn runs_count(&self, n: usize) -> String {
        format!("{n} {}", plural(n, "run", "runs"))
    }

    pub(crate) fn failed_count(&self, n: usize) -> String {
        format!("{n} failed")
    }

    pub(crate) fn issues_count(&self, n: usize) -> String {
        format!("{n} {}", plural(n, "issue", "issues"))
    }

    pub(crate) fn errors_count(&self, n: usize) -> String {
        format!("{n} {}", plural(n, "error", "errors"))
    }

    pub(crate) fn items_selected(&self, n: usize) -> String {
        format!("{} selected", self.items_count(n))
    }

    pub(crate) fn selected_of_count(&self, selected: usize, total: usize) -> String {
        format!("{selected}/{total} selected")
    }

    pub(crate) fn details_with_issues(&self, n: usize) -> String {
        format!("{} ({})", self.result_details, self.issues_count(n))
    }

    pub(crate) fn freed_summary(&self, bytes: u64) -> String {
        format!("{}{}", self.freed_prefix, human_bytes(bytes))
    }

    pub(crate) fn estimated_savings(&self, bytes: u64) -> String {
        format!("Est. savings: {}", human_bytes(bytes))
    }

    pub(crate) fn cache_scan_progress(&self, update: &ProgressUpdate) -> String {
        format!(
            "Scanning ({}/{}): {}",
            update.current, update.total, update.message
        )
    }

    pub(crate) fn cache_scan_task_progress(&self, update: &ProgressUpdate) -> String {
        format!("Scanning cache ({}/{})", update.current, update.total)
    }

    pub(crate) fn cache_delete_task_progress(&self, update: &ProgressUpdate) -> String {
        format!("Cleaning cache ({}/{})", update.current, update.total)
    }

    pub(crate) fn confirm_cache_cleanup(&self, plan: &Plan) -> String {
        format!(
            "Move {} to the Recycle Bin?\nEstimated savings: {}\n\nFiles can be restored from the Recycle Bin.",
            self.items_count(plan.selected),
            human_bytes(plan.total_bytes),
        )
    }

    pub(crate) fn cache_result_summary(&self, result: &ExecResult) -> String {
        crate::viewmodel::summary_line(&[
            &format!("{} cleaned", self.items_count(result.total_selected)),
            &format!("est. {} freed", human_bytes(result.total_bytes)),
            &format_duration(result.duration_ms),
            &self.errors_count(result.error_count),
        ])
    }
}

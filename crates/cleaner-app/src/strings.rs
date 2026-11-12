//! All user-facing text lives in one struct so the UI stays translatable and
//! the text remains easy to review.

use cleaner_core::{ExecResult, Group, GroupResult, Plan, ProgressUpdate, human_bytes};

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
    pub tooltip_clear_search: &'static str,
    pub sidebar_all: &'static str,
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
    pub tooltip_select_visible: &'static str,
    pub tooltip_expand_section: &'static str,
    pub tooltip_collapse_section: &'static str,
    pub action_expand_all: &'static str,
    pub action_collapse_all: &'static str,
    pub no_matching_cache_targets: &'static str,
    pub cache_delete_card_title: &'static str,
    pub cache_delete_card_subtitle: &'static str,

    pub status_preparing: &'static str,
    pub not_found: &'static str,

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
    pub result_status_stopped: &'static str,
    pub result_status_skipped: &'static str,
    pub result_no_matching_paths: &'static str,
    pub result_contents_only: &'static str,
    pub result_scan_issues: &'static str,
    pub result_cleanup_complete: &'static str,
    pub result_cleanup_stopped: &'static str,
    pub result_cleanup_with_errors: &'static str,
    pub freed_prefix: &'static str,

    pub history_no_runs: &'static str,
    pub history_previous_runs_title: &'static str,
    pub history_previous_runs_subtitle: &'static str,
    pub action_clear_history: &'static str,
    pub tooltip_delete_run: &'static str,
    pub dialog_clear_history_title: &'static str,
    pub dialog_clear_history_body: &'static str,

    pub run_label_duration: &'static str,
    pub run_label_items: &'static str,
    pub run_label_freed: &'static str,
    pub run_label_errors: &'static str,
    pub stats_runs_label: &'static str,
    pub stats_unreadable: &'static str,
    pub stats_total_freed_label: &'static str,
    pub stats_last7_label: &'static str,
    pub stats_last30_label: &'static str,
    pub stats_dir_unavailable: &'static str,

    pub menu_history: &'static str,
    pub menu_settings: &'static str,
    pub menu_about: &'static str,
    pub settings_title: &'static str,
    pub settings_remember: &'static str,
    pub settings_remember_hint: &'static str,
    pub settings_show_empty: &'static str,
    pub settings_show_empty_hint: &'static str,
    pub settings_reset: &'static str,
    pub settings_reset_hint: &'static str,
    pub settings_safety_note: &'static str,
    pub action_rescan_admin: &'static str,
    pub action_rescan_admin_hint: &'static str,
    pub action_show_files: &'static str,
    pub action_show_files_hint: &'static str,
    pub action_stop: &'static str,
    pub action_stop_hint: &'static str,
    pub badge_admin: &'static str,
    pub badge_admin_hint: &'static str,
    pub badge_running: &'static str,
    pub cleanup_stopping: &'static str,
    pub elevate_declined: &'static str,
    pub elevate_title: &'static str,
    pub unknown_account: &'static str,
    pub exclusion_pick_file: &'static str,
    pub exclusion_pick_folder: &'static str,
    pub files_column_modified: &'static str,
    pub files_column_path: &'static str,
    pub files_column_size: &'static str,
    pub files_exclude: &'static str,
    pub files_exclude_hint: &'static str,
    pub files_hint: &'static str,
    pub files_hint_filtered: &'static str,
    pub files_loading: &'static str,
    pub files_none: &'static str,
    pub result_stopped: &'static str,
    pub settings_add_file: &'static str,
    pub settings_add_folder: &'static str,
    pub settings_admin_title: &'static str,
    pub settings_always_elevated: &'static str,
    pub settings_always_elevated_hint: &'static str,
    pub settings_days_suffix: &'static str,
    pub settings_exclusions_hint: &'static str,
    pub settings_exclusions_none: &'static str,
    pub settings_exclusions_title: &'static str,
    pub settings_export: &'static str,
    pub settings_file_hint: &'static str,
    pub settings_file_title: &'static str,
    pub settings_import: &'static str,
    pub settings_imported: &'static str,
    pub settings_kb_suffix: &'static str,
    pub settings_limits_hint: &'static str,
    pub settings_min_age: &'static str,
    pub settings_min_size: &'static str,
    pub settings_remove: &'static str,
    pub settings_rules_title: &'static str,
    pub settings_skip_running: &'static str,
    pub settings_skip_running_hint: &'static str,

    pub presets_menu: &'static str,
    pub presets_tooltip: &'static str,
    pub presets_none: &'static str,
    pub presets_save_current: &'static str,
    pub presets_manage: &'static str,
    pub presets_save_title: &'static str,
    pub presets_save_action: &'static str,
    pub presets_name_hint: &'static str,
    pub presets_replace_note: &'static str,
    pub presets_manage_title: &'static str,
    pub presets_startup_note: &'static str,
    pub presets_on_start: &'static str,
    pub presets_on_start_hint: &'static str,
    pub presets_apply: &'static str,
    pub presets_delete: &'static str,
    pub presets_error_empty: &'static str,
    pub presets_error_duplicate: &'static str,
    pub presets_update: &'static str,
    pub presets_update_hint: &'static str,
    pub presets_status_running: &'static str,
    pub presets_status_nothing_now: &'static str,
    pub presets_status_no_empty_folders: &'static str,
    pub presets_status_gone: &'static str,
    pub presets_status_folders_on_pick: &'static str,
    pub empty_folders_details_note: &'static str,
    pub about_version_label: &'static str,
    pub about_fonts: &'static str,
    pub about_repo_label: &'static str,
    pub about_repo_url: &'static str,
    pub about_open_log: &'static str,
    pub about_updates: &'static str,
    pub about_updates_hint: &'static str,

    pub update_restart_busy: &'static str,

    pub unsupported_platform: &'static str,
    pub worker_stopped: &'static str,
}

pub(crate) const ENGLISH: UiText = UiText {
    app_title: "Windows Cleaner",

    task_cache_scanning: "Scanning cache locations…",
    task_cache_deleting: "Cleaning selected cache paths safely",
    task_history: "Cleanup history",
    task_history_failed: "Cleanup history unavailable",

    action_cancel: "Cancel",
    action_clean_up: "Clean Up",
    action_preview: "Preview",
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
    tooltip_clear_search: "Clear search",
    sidebar_all: "All",
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
    tooltip_select_visible: "Select or clear every item in view, collapsed sections included",
    tooltip_expand_section: "Expand section",
    tooltip_collapse_section: "Collapse section",
    action_expand_all: "Expand all",
    action_collapse_all: "Collapse all",
    no_matching_cache_targets: "No matching cleanup targets.",
    cache_delete_card_title: "Cleaning Up",
    cache_delete_card_subtitle: "Recycling cache paths and removing verified empty folders.",

    status_preparing: "Preparing…",
    not_found: "Not found",

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
    result_status_stopped: "Stopped",
    result_status_skipped: "Skipped",
    result_no_matching_paths: "No matching paths",
    result_contents_only: "(what it holds; the folder stays)",
    result_scan_issues: "Scan issues:",
    result_cleanup_complete: "Cleanup complete",
    result_cleanup_stopped: "Cleanup stopped",
    result_cleanup_with_errors: "Cleanup finished with errors",
    freed_prefix: "Freed: ",

    history_no_runs: "No cleanup history yet.",
    history_previous_runs_title: "Previous Cleanup Runs",
    history_previous_runs_subtitle: "Unreadable stats files are counted and skipped.",
    action_clear_history: "Clear history",
    tooltip_delete_run: "Delete this run from history",
    dialog_clear_history_title: "Clear History",
    dialog_clear_history_body: "Permanently delete all saved cleanup history? This cannot be undone.",

    run_label_duration: "Duration",
    run_label_items: "Items cleaned",
    run_label_freed: "Est. freed",
    run_label_errors: "Errors",
    stats_runs_label: "Runs",
    stats_unreadable: "unreadable",
    stats_total_freed_label: "Total freed",
    stats_last7_label: "Last 7 days",
    stats_last30_label: "Last 30 days",
    stats_dir_unavailable: "stats directory unavailable",

    menu_history: "History",
    menu_settings: "Settings",
    menu_about: "About Windows Cleaner",
    settings_title: "Settings",
    settings_remember: "Remember selected targets",
    settings_remember_hint: "Brings your last selection back after each scan.",
    settings_show_empty: "List empty targets",
    settings_show_empty_hint: "Shows targets with nothing to clean, including the apps this PC does not have.",
    settings_reset: "Reset selection to defaults",
    settings_reset_hint: "Clears what the app remembers and scans again, so the targets the catalog picks come back.",
    settings_safety_note: "Preview only is on at every launch, and empty folders are removed only when you ask for it in that run.",
    action_rescan_admin: "Scan again as administrator",
    action_rescan_admin_hint: "Some targets on this PC are under Program Files or Windows, and cleaning them needs administrator rights. This restarts Windows Cleaner with them. Windows asks first.",
    action_show_files: "Show files",
    action_show_files_hint: "Lists every file cleaning this target would move, after your exclusions and limits.",
    action_stop: "Stop",
    action_stop_hint: "Finishes the item being moved, then stops. What already moved stays in the Recycle Bin.",
    badge_admin: "Needs admin",
    badge_admin_hint: "Under Program Files or Windows. Cleaning it needs administrator rights: use Scan again as administrator.",
    badge_running: "Running",
    cleanup_stopping: "Stopping after the current item…",
    elevate_declined: "Windows did not start Windows Cleaner with administrator rights, so this window stays as it was.",
    elevate_title: "Administrator rights",
    unknown_account: "Windows Cleaner has administrator rights, but it could not confirm that it runs as the account signed in to Windows. The folders it would find might belong to another account, so it cleans nothing. Close this window and start Windows Cleaner normally.",
    exclusion_pick_file: "Choose a file to exclude",
    exclusion_pick_folder: "Choose a folder to exclude",
    files_column_modified: "Modified",
    files_column_path: "Path",
    files_column_size: "Size",
    files_exclude: "Exclude",
    files_exclude_hint: "Never clean this file. Closing the preview scans again.",
    files_hint: "A folder with nothing excluded inside is moved whole; the list shows the files it holds.",
    files_hint_filtered: "With an age or size limit set, cleanup moves these files one by one and leaves their folders.",
    files_loading: "Listing files…",
    files_none: "Nothing here would be cleaned.",
    result_stopped: "You stopped the cleanup. The last target listed may be only partly cleaned, and the targets not listed were not cleaned.",
    settings_add_file: "Add file…",
    settings_add_folder: "Add folder…",
    settings_admin_title: "Administrator rights",
    settings_always_elevated: "Always start with administrator rights",
    settings_always_elevated_hint: "Asks Windows for administrator rights each time the app starts, so system targets can be cleaned. Answering No starts it without them.",
    settings_days_suffix: " days",
    settings_exclusions_hint: "An excluded folder protects everything in it. A folder holding an exclusion is cleaned around it, never as a whole.",
    settings_exclusions_none: "Nothing is excluded.",
    settings_exclusions_title: "Exclusions",
    settings_export: "Export settings…",
    settings_file_hint: "Saves your exclusions, limits, presets, and selection preferences to a file, or loads them from one. Administrator rights are never part of the file.",
    settings_file_title: "Settings file",
    settings_import: "Import settings…",
    settings_imported: "Settings imported. Windows Cleaner scanned again with them.",
    settings_kb_suffix: " KB",
    settings_limits_hint: "With a limit set, targets are cleaned file by file and their folders stay. Zero turns a limit off. Old program versions are always cleaned whole.",
    settings_min_age: "Leave files changed in the last",
    settings_min_size: "Leave files smaller than",
    settings_remove: "Remove",
    settings_rules_title: "What to leave",
    settings_skip_running: "Skip targets while their program is running",
    settings_skip_running_hint: "A target whose program is open is not selected after a scan, and is skipped if the program starts before the cleanup reaches it.",

    presets_menu: "Presets",
    presets_tooltip: "Save the targets you have checked under a name, and check them again in one click.",
    presets_none: "No presets yet.",
    presets_save_current: "Save current selection...",
    presets_manage: "Manage presets...",
    presets_save_title: "Save current selection",
    presets_save_action: "Save",
    presets_name_hint: "Preset name",
    presets_replace_note: "A preset of this name already exists. Saving replaces what it holds.",
    presets_manage_title: "Presets",
    presets_startup_note: "The preset marked Use on start is checked for you at every launch, in place of your last selection. Its empty-folder rows are checked only when you pick it yourself.",
    presets_on_start: "Use on start",
    presets_on_start_hint: "Check this preset at every launch. Only one preset can do that.",
    presets_apply: "Apply",
    presets_delete: "Delete this preset",
    presets_error_empty: "Give the preset a name.",
    presets_error_duplicate: "Another preset already uses that name.",
    presets_update: "Update",
    presets_update_hint: "Make this preset hold the targets checked now. A target in it that this scan could not offer, such as one with nothing to clean, stays. Its name and Use on start stay too.",
    presets_status_running: "its program is running",
    presets_status_nothing_now: "nothing to clean now",
    presets_status_no_empty_folders: "no empty folders now",
    presets_status_gone: "no longer offered",
    presets_status_folders_on_pick: "checked when you pick the preset, not at launch",
    empty_folders_details_note: "Each folder is removed only if it is still empty when cleanup reaches it, the folders inside first. A folder that gained a file since the scan stays. Removed folders do not go to the Recycle Bin, because they hold nothing.",
    about_version_label: "Version",
    about_fonts: "Inter — SIL Open Font License 1.1 · Phosphor Icons — MIT",
    about_repo_label: "Project page",
    about_repo_url: "https://github.com/handsomefox/win-cleaner",
    about_open_log: "Open log folder",
    about_updates: "Check for updates",
    about_updates_hint: "Check GitHub for a new version now, and choose how often to check",

    update_restart_busy: "Wait for the scan or cleanup in progress to finish",

    unsupported_platform: "Windows Cleaner only supports Windows. Scanning and cleaning are disabled on this platform.",
    worker_stopped: "Scanning and cleaning stopped after an internal error. Restart Windows Cleaner, and attach the newest log file to an issue if it happens again.",
};

pub(crate) fn plural<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 { one } else { many }
}

// Formatting helpers take `&self` even when the English text is inlined, so a
// future locale only has to swap the receiver.
#[expect(clippy::unused_self, reason = "receiver reserved for localization")]
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

    pub(crate) fn show_empty_label(&self, n: usize) -> String {
        format!("Show empty ({n})")
    }

    /// What the Save dialog says it is about to store.
    pub(crate) fn presets_save_count(&self, n: usize) -> String {
        format!("{} checked right now.", self.items_count(n))
    }

    pub(crate) fn presets_target_count(&self, n: usize) -> String {
        self.items_count(n)
    }

    /// The first line of a preset's contents: how many of its targets
    /// picking it checks on this scan.
    pub(crate) fn presets_apply_summary(&self, checks: usize, total: usize) -> String {
        if checks == total {
            format!("Checks the {} in this preset.", self.items_count(total))
        } else {
            format!(
                "Checks {checks} of the {} in this preset right now.",
                self.items_count(total)
            )
        }
    }

    /// The last line of a list cut short.
    pub(crate) fn and_more(&self, n: usize) -> String {
        format!("and {n} more")
    }

    /// What an empty-folder row shows in place of a size.
    pub(crate) fn empty_folder_count(&self, n: usize) -> String {
        format!("{n} empty {}", plural(n, "folder", "folders"))
    }

    pub(crate) fn presets_error_full(&self, max: usize) -> String {
        format!(
            "You can keep {max} presets. Delete one first, or save over a name you already have."
        )
    }

    /// The status-bar overview line: apps scanned, cleanup targets, and their
    /// combined estimated size.
    pub(crate) fn cache_overview(&self, apps: usize, items: usize, bytes: u64) -> String {
        format!(
            "{}  ·  {}  ·  {}",
            self.apps_count(apps),
            self.items_count(items),
            human_bytes(bytes)
        )
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
            "Clean {}?\nEstimated savings: {}\n\nCache paths are moved to the Recycle Bin. Empty folders are removed only if they are still empty.",
            self.items_count(plan.selected),
            human_bytes(plan.total_bytes),
        )
    }

    /// The total under the preview dialog's list.
    pub(crate) fn preview_total(&self, plan: &Plan) -> String {
        format!(
            "{}  |  est. {}",
            self.items_count(plan.selected),
            human_bytes(plan.total_bytes)
        )
    }

    /// The plain-text detail body for one plan group (paths + scan issues).
    pub(crate) fn group_details(&self, group: &Group) -> String {
        use std::fmt::Write as _;

        if group.app == cleaner_core::EMPTY_FOLDERS_APP {
            return self.empty_folder_details(group);
        }
        let mut out = String::new();
        let _ = writeln!(
            out,
            "App: {}\nTarget: {}\nEstimated size: {}\n\nPaths:",
            group.app,
            group.label,
            human_bytes(group.bytes)
        );
        if group.targets.is_empty() {
            let _ = writeln!(out, "- {}", self.result_no_matching_paths);
        }
        for target in &group.targets {
            if target.keep_folder {
                let _ = writeln!(
                    out,
                    "- {} {}",
                    target.path.display(),
                    self.result_contents_only
                );
            } else {
                let _ = writeln!(out, "- {}", target.path.display());
            }
        }
        self.scan_issues(&mut out, group);
        out
    }

    /// The detail body for an empty-folder row: what cleanup removes, and
    /// each empty folder at the top of its tree with how many it holds.
    fn empty_folder_details(&self, group: &Group) -> String {
        use std::fmt::Write as _;

        let mut out = String::new();
        let _ = writeln!(
            out,
            "Inside: {}\nRemoves: {}\n\n{}\n\nFolders:",
            group.label,
            self.empty_folder_count(group.targets.len()),
            self.empty_folders_details_note
        );
        for top in crate::viewmodel::empty_folder_tops(group) {
            let inside = group
                .targets
                .iter()
                .filter(|target| target.path != top && target.path.starts_with(top))
                .count();
            if inside == 0 {
                let _ = writeln!(out, "- {}", top.display());
            } else {
                let _ = writeln!(
                    out,
                    "- {} (and the {} inside)",
                    top.display(),
                    self.empty_folder_count(inside)
                );
            }
        }
        self.scan_issues(&mut out, group);
        out
    }

    fn scan_issues(&self, out: &mut String, group: &Group) {
        use std::fmt::Write as _;

        if !group.errs.is_empty() {
            let _ = writeln!(out, "\n{}", self.result_scan_issues);
            for err in &group.errs {
                let _ = writeln!(out, "- {err}");
            }
        }
    }

    /// The plain-text detail body for one group of a finished cleanup.
    pub(crate) fn result_group_details(&self, group: &GroupResult) -> String {
        use std::fmt::Write as _;

        let mut out = String::new();
        let _ = writeln!(
            out,
            "{} - {}\nEstimated size: {}\nPaths attempted: {}\nPaths failed: {}",
            group.app,
            group.label,
            human_bytes(group.bytes),
            group.paths_attempted,
            group.paths_failed
        );
        if !group.errors.is_empty() {
            out.push_str("\nErrors:\n");
            for error in &group.errors {
                let _ = writeln!(out, "- {}: {}", error.path, error.error);
            }
        }
        out
    }

    /// The results screen's error line: how many targets failed, and the
    /// first few of their names.
    pub(crate) fn errors_in(&self, groups: usize, names: &str) -> String {
        format!("Errors in {}: {names}", self.items_count(groups))
    }

    pub(crate) fn badge_running_hint(&self, name: &str) -> String {
        format!(
            "{name} is running, so this target is skipped. Close it and scan again to clean it."
        )
    }

    pub(crate) fn elevate_failed(&self, error: &str) -> String {
        format!("Could not start Windows Cleaner with administrator rights: {error}")
    }

    pub(crate) fn files_summary(&self, files: usize, bytes: u64) -> String {
        let noun = if files == 1 { "file" } else { "files" };
        format!("{files} {noun}, {}", human_bytes(bytes))
    }

    pub(crate) fn files_truncated(&self, limit: usize) -> String {
        format!("Only the first {limit} files are listed.")
    }

    pub(crate) fn other_account(&self, process: &str, session: &str) -> String {
        format!(
            "Windows Cleaner is running as {process}, but {session} is signed in. Windows gave it \
             administrator rights through another account, so the folders it finds belong to \
             {process}, not to you, and it cleans nothing. Close this window and start Windows \
             Cleaner normally."
        )
    }

    pub(crate) fn result_status_skipped_because(&self, reason: &str) -> String {
        format!("Skipped: {reason}")
    }

    pub(crate) fn settings_exported(&self, path: &std::path::Path) -> String {
        format!("Saved to {}.", path.display())
    }

    pub(crate) fn settings_export_failed(&self, error: &str) -> String {
        format!("Could not save the settings file: {error}")
    }

    pub(crate) fn settings_import_failed(&self, error: &str) -> String {
        format!("Could not read the settings file: {error}")
    }

    pub(crate) fn settings_import_refused(
        &self,
        error: &crate::settings_file::ImportError,
    ) -> String {
        use crate::settings_file::ImportError;
        match error {
            ImportError::TooLarge => "That file is too large to be a settings file.".to_owned(),
            ImportError::NotJson(detail) => {
                format!("That file is not a settings file Windows Cleaner can read: {detail}")
            }
            ImportError::WrongFormat => {
                "That file holds settings for something other than Windows Cleaner.".to_owned()
            }
            ImportError::NewerVersion(version) => format!(
                "That file was written by a newer Windows Cleaner (format {version}). Update this one first."
            ),
        }
    }

    pub(crate) fn stats_write_failed(&self, error: &str) -> String {
        format!("Failed to write stats: {error}")
    }

    pub(crate) fn cache_result_summary(&self, result: &ExecResult) -> String {
        crate::viewmodel::summary_line(&[
            &format!("{} cleaned", self.items_count(result.cleaned_count())),
            &match result.skipped_count() {
                0 => String::new(),
                skipped => format!("{skipped} skipped"),
            },
            &format!("est. {} freed", human_bytes(result.freed())),
            &format_duration(result.duration_ms),
            &self.errors_count(result.error_count),
        ])
    }
}

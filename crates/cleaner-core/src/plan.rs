use std::path::PathBuf;

/// Execution options.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// When set, nothing is deleted; the plan is only previewed.
    pub dry_run: bool,
}

/// A scanned cleanup plan: one [`Group`] per catalog item (plus empty-folder
/// groups), with selection state and reclaimable-size totals.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub groups: Vec<Group>,
    /// Total bytes across selected groups.
    pub total_bytes: u64,
    /// Number of selected groups.
    pub selected: usize,
}

impl Plan {
    /// Recomputes `total_bytes` and `selected` from the groups' `on` flags.
    pub fn recompute_totals(&mut self) {
        let mut total = 0u64;
        let mut selected = 0usize;
        for group in &self.groups {
            if group.on {
                selected += 1;
                total += group.bytes;
            }
        }
        self.total_bytes = total;
        self.selected = selected;
    }

    /// Groups plan entries by app name, sorted alphabetically. Entries are
    /// returned as indices into `self.groups` so callers can mutate selection
    /// state without holding references.
    #[must_use]
    pub fn by_app(&self) -> Vec<AppGroup> {
        let mut order: Vec<&str> = Vec::new();
        for group in &self.groups {
            if !order.contains(&group.app.as_str()) {
                order.push(&group.app);
            }
        }
        order.sort_unstable();

        order
            .into_iter()
            .map(|app| {
                let indices: Vec<usize> = self
                    .groups
                    .iter()
                    .enumerate()
                    .filter(|(_, g)| g.app == app)
                    .map(|(i, _)| i)
                    .collect();
                let bytes = indices.iter().map(|&i| self.groups[i].bytes).sum();
                AppGroup {
                    app: app.to_owned(),
                    indices,
                    bytes,
                }
            })
            .collect()
    }
}

/// A single selectable cleanup target with its resolved paths.
#[derive(Debug, Clone)]
pub struct Group {
    pub app: String,
    pub label: String,
    /// Resolved paths (globs expanded).
    pub paths: Vec<PathBuf>,
    /// Non-fatal problems encountered while scanning this group.
    pub errs: Vec<String>,
    /// Estimated reclaimable bytes.
    pub bytes: u64,
    /// Selected for deletion.
    pub on: bool,
}

/// All groups for one app, as indices into [`Plan::groups`].
#[derive(Debug, Clone)]
pub struct AppGroup {
    pub app: String,
    pub indices: Vec<usize>,
    /// Sum of all the app's groups.
    pub bytes: u64,
}

/// Phase reported by progress callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Scan,
    Delete,
}

/// Progress reported while scanning or deleting.
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub phase: Phase,
    pub message: String,
    pub current: usize,
    pub total: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn recompute_totals_counts_only_selected() {
        let mut plan = Plan {
            groups: vec![
                group("A", "one", 10, true),
                group("A", "two", 20, false),
                group("B", "three", 30, true),
            ],
            ..Plan::default()
        };
        plan.recompute_totals();
        assert_eq!(plan.selected, 2);
        assert_eq!(plan.total_bytes, 40);
    }

    #[test]
    fn by_app_groups_and_sorts() {
        let plan = Plan {
            groups: vec![
                group("Zeta", "z", 1, false),
                group("Alpha", "a1", 2, false),
                group("Alpha", "a2", 3, false),
            ],
            ..Plan::default()
        };
        let apps = plan.by_app();
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].app, "Alpha");
        assert_eq!(apps[0].indices, vec![1, 2]);
        assert_eq!(apps[0].bytes, 5);
        assert_eq!(apps[1].app, "Zeta");
        assert_eq!(apps[1].indices, vec![0]);
    }
}

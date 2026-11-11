//! Carrying saved selections over to targets that were renamed.
//!
//! 1.3.0 folds Chrome's release channels into one Chrome entry: the targets
//! once named `Chrome Beta - Profile cache` are now `Chrome - Profile cache
//! (Beta)`, and so on. Remembered selections and presets still hold the old
//! names, so they are rewritten as they load. Each old key maps to exactly
//! one new key, so nothing that was off turns on.

use crate::viewmodel::TargetKey;

/// Old app and label, then new app and label.
const RENAMED: &[(&str, &str, &str, &str)] = &[
    (
        "Chrome Beta",
        "Profile cache",
        "Chrome",
        "Profile cache (Beta)",
    ),
    (
        "Chrome Dev",
        "Profile cache",
        "Chrome",
        "Profile cache (Dev)",
    ),
    (
        "Chrome Canary",
        "Profile cache",
        "Chrome",
        "Profile cache (Canary)",
    ),
];

/// Rewrites renamed targets in `keys`, in place, and drops any repeat the
/// rewrite creates.
pub(crate) fn target_keys(keys: &mut Vec<TargetKey>) {
    for key in keys.iter_mut() {
        if let Some((_, _, app, label)) = RENAMED
            .iter()
            .find(|(old_app, old_label, _, _)| key.0 == *old_app && key.1 == *old_label)
        {
            *key = ((*app).to_owned(), (*label).to_owned());
        }
    }
    let mut seen = std::collections::HashSet::new();
    keys.retain(|key| seen.insert(key.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(app: &str, label: &str) -> TargetKey {
        (app.to_owned(), label.to_owned())
    }

    #[test]
    fn channel_targets_move_into_the_browser_entry() {
        let mut keys = vec![
            key("Chrome", "Profile cache"),
            key("Chrome Beta", "Profile cache"),
            key("Chrome Canary", "Profile cache"),
            key("Steam", "HTML cache"),
        ];
        target_keys(&mut keys);
        assert_eq!(
            keys,
            vec![
                key("Chrome", "Profile cache"),
                key("Chrome", "Profile cache (Beta)"),
                key("Chrome", "Profile cache (Canary)"),
                key("Steam", "HTML cache"),
            ],
            "Dev was off and stays off; the rest keep their state"
        );
    }

    #[test]
    fn a_key_already_in_the_new_form_is_not_repeated() {
        let mut keys = vec![
            key("Chrome Dev", "Profile cache"),
            key("Chrome", "Profile cache (Dev)"),
        ];
        target_keys(&mut keys);
        assert_eq!(keys, vec![key("Chrome", "Profile cache (Dev)")]);
    }
}

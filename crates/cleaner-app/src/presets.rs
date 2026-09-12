//! Named selections the user saves, applies, and picks a startup default
//! from. Toolkit-agnostic, so every rule here is unit-tested without a GUI.
//!
//! A preset holds cleanup targets by key, never categories, so it survives a
//! catalog that regroups its items. Nothing in this module edits a preset on
//! the user's behalf: unlike the remembered selection, which follows what the
//! selection screen shows, a preset changes only when the user says so.

use serde::{Deserialize, Serialize};

use crate::viewmodel::TargetKey;

/// How many presets one install keeps, so the menu stays readable and the
/// stored list stays small. Saving a new name past this fails; saving over a
/// name already in the list still works.
pub(crate) const MAX_PRESETS: usize = 20;

/// One saved selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Preset {
    pub name: String,
    pub keys: Vec<TargetKey>,
    /// Applied at every launch in place of the last selection. At most one
    /// preset carries it.
    #[serde(default)]
    pub on_start: bool,
}

/// Why a preset could not be saved or renamed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PresetError {
    EmptyName,
    DuplicateName,
    Full,
}

/// The saved presets, in the order they appear in the menu.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Presets(Vec<Preset>);

impl Presets {
    pub(crate) fn as_slice(&self) -> &[Preset] {
        &self.0
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn get(&self, index: usize) -> Option<&Preset> {
        self.0.get(index)
    }

    /// The preset applied at launch, if the user marked one.
    pub(crate) fn on_start(&self) -> Option<&Preset> {
        self.0.iter().find(|preset| preset.on_start)
    }

    /// Whether a preset under this name already exists, so the UI can offer to
    /// replace rather than add.
    pub(crate) fn position(&self, name: &str) -> Option<usize> {
        let name = name.trim();
        self.0
            .iter()
            .position(|preset| preset.name.eq_ignore_ascii_case(name))
    }

    /// Stores `keys` under `name`, replacing a preset of the same name and
    /// keeping whether that one was the startup preset.
    pub(crate) fn save(&mut self, name: &str, keys: Vec<TargetKey>) -> Result<usize, PresetError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(PresetError::EmptyName);
        }
        if let Some(index) = self.position(name) {
            name.clone_into(&mut self.0[index].name);
            self.0[index].keys = keys;
            return Ok(index);
        }
        if self.0.len() >= MAX_PRESETS {
            return Err(PresetError::Full);
        }
        self.0.push(Preset {
            name: name.to_owned(),
            keys,
            on_start: false,
        });
        Ok(self.0.len() - 1)
    }

    pub(crate) fn rename(&mut self, index: usize, name: &str) -> Result<(), PresetError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(PresetError::EmptyName);
        }
        if self.position(name).is_some_and(|found| found != index) {
            return Err(PresetError::DuplicateName);
        }
        if let Some(preset) = self.0.get_mut(index) {
            name.clone_into(&mut preset.name);
        }
        Ok(())
    }

    pub(crate) fn remove(&mut self, index: usize) {
        if index < self.0.len() {
            self.0.remove(index);
        }
    }

    /// Marks one preset as the startup selection, clearing the others so the
    /// launch path never has to choose between two.
    pub(crate) fn set_on_start(&mut self, index: usize, on: bool) {
        for (position, preset) in self.0.iter_mut().enumerate() {
            preset.on_start = on && position == index;
        }
    }

    /// Drops what storage should not have handed back: nameless presets,
    /// duplicates, anything past the cap, and a second startup preset.
    pub(crate) fn sanitize(&mut self) {
        let mut seen: Vec<String> = Vec::new();
        let mut startup_taken = false;
        self.0.retain_mut(|preset| {
            preset.name = preset.name.trim().to_owned();
            if preset.name.is_empty() {
                return false;
            }
            let lowered = preset.name.to_lowercase();
            if seen.contains(&lowered) {
                return false;
            }
            seen.push(lowered);
            if preset.on_start {
                if startup_taken {
                    preset.on_start = false;
                } else {
                    startup_taken = true;
                }
            }
            true
        });
        self.0.truncate(MAX_PRESETS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(names: &[&str]) -> Vec<TargetKey> {
        names
            .iter()
            .map(|name| ((*name).to_owned(), "cache".to_owned()))
            .collect()
    }

    fn preset(name: &str, on_start: bool) -> Preset {
        Preset {
            name: name.to_owned(),
            keys: keys(&["Chrome"]),
            on_start,
        }
    }

    #[test]
    fn saving_adds_then_replaces_by_name() {
        let mut presets = Presets::default();
        assert_eq!(presets.save("Weekly", keys(&["Chrome"])), Ok(0));
        assert_eq!(presets.save("Dev caches", keys(&["npm"])), Ok(1));
        assert_eq!(presets.as_slice().len(), 2);

        // The same name replaces rather than piling up a second entry.
        presets.set_on_start(0, true);
        assert_eq!(presets.save(" weekly ", keys(&["Edge", "Brave"])), Ok(0));
        assert_eq!(presets.as_slice().len(), 2);
        assert_eq!(presets.get(0).unwrap().keys, keys(&["Edge", "Brave"]));
        assert!(
            presets.get(0).unwrap().on_start,
            "replacing keeps it the startup preset"
        );
        assert_eq!(presets.get(0).unwrap().name, "weekly", "the new name wins");
    }

    #[test]
    fn saving_rejects_an_empty_name_and_a_full_list() {
        let mut presets = Presets::default();
        assert_eq!(
            presets.save("   ", keys(&["Chrome"])),
            Err(PresetError::EmptyName)
        );
        assert!(presets.is_empty());

        for n in 0..MAX_PRESETS {
            assert!(presets.save(&format!("p{n}"), Vec::new()).is_ok());
        }
        assert_eq!(presets.save("one more", Vec::new()), Err(PresetError::Full));
        // Replacing an existing name still works at the cap.
        assert_eq!(presets.save("p0", keys(&["npm"])), Ok(0));
    }

    #[test]
    fn renaming_rejects_empty_and_duplicate_names() {
        let mut presets = Presets::default();
        presets.save("Weekly", Vec::new()).unwrap();
        presets.save("Dev", Vec::new()).unwrap();

        assert_eq!(presets.rename(1, " "), Err(PresetError::EmptyName));
        assert_eq!(presets.rename(1, "weekly"), Err(PresetError::DuplicateName));
        assert_eq!(
            presets.get(1).unwrap().name,
            "Dev",
            "a refusal changes nothing"
        );
        assert_eq!(presets.rename(1, "Dev caches"), Ok(()));
        assert_eq!(presets.get(1).unwrap().name, "Dev caches");
        // Renaming to its own name, in another case, is not a duplicate.
        assert_eq!(presets.rename(1, "DEV CACHES"), Ok(()));
    }

    #[test]
    fn only_one_preset_starts_the_app() {
        let mut presets = Presets::default();
        presets.save("Weekly", Vec::new()).unwrap();
        presets.save("Dev", Vec::new()).unwrap();
        assert!(presets.on_start().is_none(), "none is the default");

        presets.set_on_start(0, true);
        assert_eq!(presets.on_start().unwrap().name, "Weekly");
        presets.set_on_start(1, true);
        assert_eq!(
            presets.on_start().unwrap().name,
            "Dev",
            "picking one clears the other"
        );
        presets.set_on_start(1, false);
        assert!(presets.on_start().is_none());
    }

    #[test]
    fn removing_keeps_the_rest_and_ignores_a_bad_index() {
        let mut presets = Presets::default();
        presets.save("Weekly", Vec::new()).unwrap();
        presets.save("Dev", Vec::new()).unwrap();
        presets.remove(9);
        assert_eq!(presets.as_slice().len(), 2);
        presets.remove(0);
        assert_eq!(presets.as_slice().len(), 1);
        assert_eq!(presets.get(0).unwrap().name, "Dev");
    }

    #[test]
    fn sanitize_drops_what_storage_should_not_return() {
        let mut presets = Presets(vec![
            preset("  Weekly  ", true),
            preset("", false),
            // A duplicate name, whatever its case.
            preset("weekly", false),
            preset("Dev", true),
        ]);
        presets.sanitize();

        let names: Vec<&str> = presets
            .as_slice()
            .iter()
            .map(|preset| preset.name.as_str())
            .collect();
        assert_eq!(
            names,
            vec!["Weekly", "Dev"],
            "trimmed, no blanks, no duplicates"
        );
        assert_eq!(
            presets.on_start().unwrap().name,
            "Weekly",
            "the first startup preset keeps the mark"
        );
        assert!(!presets.get(1).unwrap().on_start);
    }

    #[test]
    fn sanitize_enforces_the_cap() {
        let mut presets = Presets(
            (0..MAX_PRESETS + 5)
                .map(|n| preset(&format!("p{n}"), false))
                .collect(),
        );
        presets.sanitize();
        assert_eq!(presets.as_slice().len(), MAX_PRESETS);
    }
}

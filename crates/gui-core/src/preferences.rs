// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! GUI-only preferences stored separately from tunnel profiles.

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const CURRENT_UI_PREFERENCES_VERSION: u32 = 1;

fn current_version() -> u32 {
    CURRENT_UI_PREFERENCES_VERSION
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortMode {
    #[default]
    Manual,
    Name,
}

/// Preferences that affect presentation only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPreferences {
    #[serde(default = "current_version")]
    pub version: u32,
    pub profile_order: Vec<Uuid>,
    pub pinned_profiles: Vec<Uuid>,
    pub connected_only: bool,
    pub sort_mode: SortMode,
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            version: CURRENT_UI_PREFERENCES_VERSION,
            profile_order: Vec::new(),
            pinned_profiles: Vec::new(),
            connected_only: false,
            sort_mode: SortMode::Manual,
        }
    }
}

impl UiPreferences {
    /// Reconcile stored UUIDs with the currently available profiles.
    ///
    /// Unknown and duplicate IDs are removed, while new profiles are appended
    /// in the order supplied by the profile repository.
    pub fn reconcile_profiles<I>(&mut self, profile_ids: I) -> bool
    where
        I: IntoIterator<Item = Uuid>,
    {
        let profile_ids: Vec<Uuid> = profile_ids.into_iter().collect();
        let known: HashSet<Uuid> = profile_ids.iter().copied().collect();
        let before_order = self.profile_order.clone();
        let before_pinned = self.pinned_profiles.clone();

        deduplicate_and_retain(&mut self.profile_order, &known);
        deduplicate_and_retain(&mut self.pinned_profiles, &known);

        let mut ordered: HashSet<Uuid> = self.profile_order.iter().copied().collect();
        for profile_id in profile_ids {
            if ordered.insert(profile_id) {
                self.profile_order.push(profile_id);
            }
        }

        self.version = CURRENT_UI_PREFERENCES_VERSION;
        self.profile_order != before_order || self.pinned_profiles != before_pinned
    }

    pub fn is_pinned(&self, profile_id: Uuid) -> bool {
        self.pinned_profiles.contains(&profile_id)
    }

    pub fn set_pinned(&mut self, profile_id: Uuid, pinned: bool) -> bool {
        let already_pinned = self.is_pinned(profile_id);
        if pinned == already_pinned {
            return false;
        }

        if pinned {
            self.pinned_profiles.push(profile_id);
        } else {
            self.pinned_profiles.retain(|id| *id != profile_id);
        }
        true
    }

    pub fn set_connected_only(&mut self, connected_only: bool) -> bool {
        if self.connected_only == connected_only {
            return false;
        }

        self.connected_only = connected_only;
        true
    }

    pub fn set_sort_mode(&mut self, sort_mode: SortMode) -> bool {
        if self.sort_mode == sort_mode {
            return false;
        }

        self.sort_mode = sort_mode;
        true
    }

    pub fn move_profile(&mut self, profile_id: Uuid, new_index: usize) -> bool {
        let Some(old_index) = self.profile_order.iter().position(|id| *id == profile_id) else {
            return false;
        };

        let profile_id = self.profile_order.remove(old_index);
        let bounded_index = new_index.min(self.profile_order.len());
        self.profile_order.insert(bounded_index, profile_id);
        old_index != bounded_index
    }

    pub fn remove_profile(&mut self, profile_id: Uuid) -> bool {
        let order_len = self.profile_order.len();
        let pinned_len = self.pinned_profiles.len();
        self.profile_order.retain(|id| *id != profile_id);
        self.pinned_profiles.retain(|id| *id != profile_id);
        order_len != self.profile_order.len() || pinned_len != self.pinned_profiles.len()
    }

    /// Return IDs in display order. Pinned profiles come first while retaining
    /// their relative order.
    pub fn display_order(&self) -> Vec<Uuid> {
        let pinned: HashSet<Uuid> = self.pinned_profiles.iter().copied().collect();
        let mut result = Vec::with_capacity(self.profile_order.len());
        result.extend(
            self.profile_order
                .iter()
                .filter(|id| pinned.contains(id))
                .copied(),
        );
        result.extend(
            self.profile_order
                .iter()
                .filter(|id| !pinned.contains(id))
                .copied(),
        );
        result
    }
}

fn deduplicate_and_retain(values: &mut Vec<Uuid>, known: &HashSet<Uuid>) {
    let mut seen = HashSet::new();
    values.retain(|id| known.contains(id) && seen.insert(*id));
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiPreferencesLoad {
    pub preferences: UiPreferences,
    /// Recoverable parsing/version warning for presentation to the user.
    pub warning: Option<String>,
}

/// Repository for `${XDG_CONFIG_HOME}/ssh-tunnel-manager/ui.toml`.
#[derive(Debug, Clone)]
pub struct UiPreferencesRepository {
    path: PathBuf,
}

impl UiPreferencesRepository {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_path() -> Result<PathBuf> {
        let config_dir =
            dirs::config_dir().context("Could not determine the user configuration directory")?;
        Ok(config_dir.join("ssh-tunnel-manager").join("ui.toml"))
    }

    pub fn for_default_path() -> Result<Self> {
        Ok(Self::new(Self::default_path()?))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Load preferences. Missing, malformed, or unsupported-version files use
    /// safe defaults; malformed and unsupported files return a warning.
    pub fn load(&self) -> Result<UiPreferencesLoad> {
        let contents = match fs::read_to_string(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(UiPreferencesLoad {
                    preferences: UiPreferences::default(),
                    warning: None,
                });
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to read {}", self.path.display()));
            }
        };

        let preferences: UiPreferences = match toml::from_str(&contents) {
            Ok(preferences) => preferences,
            Err(error) => {
                return Ok(UiPreferencesLoad {
                    preferences: UiPreferences::default(),
                    warning: Some(format!(
                        "Could not parse GUI preferences at {}: {error}",
                        self.path.display()
                    )),
                });
            }
        };

        if preferences.version != CURRENT_UI_PREFERENCES_VERSION {
            return Ok(UiPreferencesLoad {
                preferences: UiPreferences::default(),
                warning: Some(format!(
                    "Unsupported GUI preferences version {} at {}; expected {}",
                    preferences.version,
                    self.path.display(),
                    CURRENT_UI_PREFERENCES_VERSION
                )),
            });
        }

        Ok(UiPreferencesLoad {
            preferences,
            warning: None,
        })
    }

    /// Atomically persist preferences by writing and syncing a sibling temp
    /// file before renaming it over the destination.
    pub fn save(&self, preferences: &UiPreferences) -> Result<()> {
        let parent = self.path.parent().with_context(|| {
            format!(
                "GUI preferences path has no parent: {}",
                self.path.display()
            )
        })?;
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;

        let mut persisted = preferences.clone();
        persisted.version = CURRENT_UI_PREFERENCES_VERSION;
        let serialized =
            toml::to_string_pretty(&persisted).context("Failed to serialize GUI preferences")?;

        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("ui.toml");
        let temporary_path = parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            Uuid::new_v4()
        ));

        let save_result = (|| -> Result<()> {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary_path)
                .with_context(|| format!("Failed to create {}", temporary_path.display()))?;
            file.write_all(serialized.as_bytes())
                .with_context(|| format!("Failed to write {}", temporary_path.display()))?;
            file.sync_all()
                .with_context(|| format!("Failed to sync {}", temporary_path.display()))?;
            drop(file);

            fs::rename(&temporary_path, &self.path).with_context(|| {
                format!(
                    "Failed to replace {} with {}",
                    self.path.display(),
                    temporary_path.display()
                )
            })?;

            // Best effort directory sync: the data file is already safely in place.
            if let Ok(directory) = File::open(parent) {
                let _ = directory.sync_all();
            }
            Ok(())
        })();

        if save_result.is_err() {
            let _ = fs::remove_file(&temporary_path);
        }
        save_result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "ssh-tunnel-gui-core-preferences-{}-{}",
                std::process::id(),
                Uuid::new_v4()
            ));
            fs::create_dir_all(&path).expect("test directory should be created");
            Self { path }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn missing_preferences_use_defaults_without_warning() {
        let directory = TestDirectory::new();
        let repository = UiPreferencesRepository::new(directory.path.join("ui.toml"));
        let loaded = repository
            .load()
            .expect("missing file should be recoverable");

        assert_eq!(loaded.preferences, UiPreferences::default());
        assert_eq!(loaded.warning, None);
    }

    #[test]
    fn preferences_round_trip_through_atomic_save() {
        let directory = TestDirectory::new();
        let repository = UiPreferencesRepository::new(directory.path.join("ui.toml"));
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let preferences = UiPreferences {
            profile_order: vec![second, first],
            pinned_profiles: vec![first],
            connected_only: true,
            sort_mode: SortMode::Name,
            ..UiPreferences::default()
        };

        repository
            .save(&preferences)
            .expect("preferences should save");
        let loaded = repository.load().expect("preferences should load");

        assert_eq!(loaded.preferences, preferences);
        assert_eq!(loaded.warning, None);
        let files: Vec<_> = fs::read_dir(&directory.path)
            .expect("directory should be readable")
            .collect();
        assert_eq!(files.len(), 1, "atomic save should not leave temp files");
    }

    #[test]
    fn malformed_preferences_fail_safely_with_warning() {
        let directory = TestDirectory::new();
        let path = directory.path.join("ui.toml");
        fs::write(&path, "this is not = valid = toml")
            .expect("malformed fixture should be written");
        let loaded = UiPreferencesRepository::new(path)
            .load()
            .expect("malformed preferences should be recoverable");

        assert_eq!(loaded.preferences, UiPreferences::default());
        assert!(loaded.warning.is_some());
    }

    #[test]
    fn reconciliation_removes_stale_and_duplicate_ids_and_appends_new_ids() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let new_profile = Uuid::new_v4();
        let stale = Uuid::new_v4();
        let mut preferences = UiPreferences {
            profile_order: vec![second, stale, second, first],
            pinned_profiles: vec![stale, first, first],
            ..UiPreferences::default()
        };

        assert!(preferences.reconcile_profiles([first, second, new_profile]));
        assert_eq!(preferences.profile_order, vec![second, first, new_profile]);
        assert_eq!(preferences.pinned_profiles, vec![first]);
        assert_eq!(
            preferences.display_order(),
            vec![first, second, new_profile]
        );
    }

    #[test]
    fn move_and_pin_are_deterministic() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let third = Uuid::new_v4();
        let mut preferences = UiPreferences {
            profile_order: vec![first, second, third],
            ..UiPreferences::default()
        };

        assert!(preferences.move_profile(third, 0));
        assert!(preferences.set_pinned(second, true));
        assert_eq!(preferences.profile_order, vec![third, first, second]);
        assert_eq!(preferences.display_order(), vec![second, third, first]);
    }

    #[test]
    fn filter_and_sort_setters_report_real_changes_only() {
        let mut preferences = UiPreferences::default();

        assert!(!preferences.set_connected_only(false));
        assert!(preferences.set_connected_only(true));
        assert!(preferences.connected_only);

        assert!(!preferences.set_sort_mode(SortMode::Manual));
        assert!(preferences.set_sort_mode(SortMode::Name));
        assert_eq!(preferences.sort_mode, SortMode::Name);
    }
}

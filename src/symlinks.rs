//! Symlink management module for dotme
//!
//! This module provides functionality to create, manage, and track symlinks with persistent
//! state stored in `~/.dotme/symlinks.yml`. It ensures that symlinks are properly verified
//! before creation and that the state file is kept synchronized with the actual filesystem.
//!
//! # Features
//!
//! - **State Tracking**: All symlinks are tracked in `~/.dotme/symlinks.yml`
//! - **Verification**: Verifies filesystem state before operations
//! - **Timestamps**: Tracks creation and last verification time
//! - **Cross-platform**: Supports both Unix and Windows
//! - **Safe Operations**: Prevents overwriting existing files/directories
//!
//! # Example Usage
//!
//! ```no_run
//! use dotme::symlinks::{create_symlink, remove_symlink, list_symlinks};
//! use std::path::Path;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Create a symlink
//!     let target = Path::new("/home/user/dotfiles/bashrc");
//!     let link = Path::new("/home/user/.bashrc");
//!     create_symlink(link, target).await?;
//!
//!     // List all managed symlinks
//!     let symlinks = list_symlinks().await?;
//!     for (entry, status) in symlinks {
//!         println!("{:?} -> {:?}: {:?}", entry.link, entry.target, status);
//!     }
//!
//!     // Remove a symlink
//!     remove_symlink(link).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # State File Format
//!
//! The state is stored in `~/.dotme/symlinks.yml` in YAML format:
//!
//! ```yaml
//! symlinks:
//!   - link: "/home/user/.bashrc"
//!     target: "/home/user/dotfiles/bashrc"
//!     created_at: "2024-01-15T10:30:00Z"
//!     last_verified: "2024-01-15T12:45:00Z"
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Represents a single symlink entry in the state file
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SymlinkEntry {
    /// The path to the symlink itself
    pub link: PathBuf,
    /// The target that the symlink points to
    pub target: PathBuf,
    /// Timestamp when the symlink was created (ISO 8601 format)
    pub created_at: String,
    /// Last verified timestamp (ISO 8601 format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_verified: Option<String>,
}

/// State manager for all symlinks created by dotme
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SymlinkState {
    /// List of managed symlinks
    #[serde(default)]
    pub symlinks: Vec<SymlinkEntry>,
}

impl SymlinkState {
    /// Load symlink state from ~/.dotme/symlinks.yml
    pub async fn load() -> Result<Self> {
        let path = Self::get_state_path()?;

        if !path.exists() {
            log::debug!("Symlink state file does not exist, returning empty state");
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .await
            .context("Failed to read symlink state file")?;

        let state: Self =
            serde_yaml::from_str(&contents).context("Failed to parse symlink state file")?;

        log::debug!("Loaded {} symlink entries from state", state.symlinks.len());

        Ok(state)
    }

    /// Save symlink state to ~/.dotme/symlinks.yml
    pub async fn save(&self) -> Result<()> {
        let path = Self::get_state_path()?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create .dotme directory")?;
        }

        let contents = serde_yaml::to_string(self).context("Failed to serialize symlink state")?;

        fs::write(&path, contents)
            .await
            .context("Failed to write symlink state file")?;

        log::debug!("Saved {} symlink entries to state", self.symlinks.len());

        Ok(())
    }

    /// Get the path to the symlink state file
    fn get_state_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Failed to get home directory")?;
        Ok(home.join(".dotme").join("symlinks.yml"))
    }

    /// Add a new symlink entry to the state
    pub fn add_entry(&mut self, link: PathBuf, target: PathBuf) {
        let now = chrono::Utc::now().to_rfc3339();

        // Check if entry already exists and update it
        if let Some(entry) = self.symlinks.iter_mut().find(|e| e.link == link) {
            entry.target = target;
            entry.last_verified = Some(now);
            log::debug!("Updated existing symlink entry: {:?}", link);
        } else {
            let entry = SymlinkEntry {
                link,
                target,
                created_at: now.clone(),
                last_verified: Some(now),
            };
            self.symlinks.push(entry);
            log::debug!("Added new symlink entry");
        }
    }

    /// Remove a symlink entry from the state by link path
    pub fn remove_entry(&mut self, link: &Path) -> bool {
        let before = self.symlinks.len();
        self.symlinks.retain(|e| e.link != link);
        let removed = before != self.symlinks.len();

        if removed {
            log::debug!("Removed symlink entry: {:?}", link);
        }

        removed
    }

    /// Find a symlink entry by link path
    pub fn find_entry(&self, link: &Path) -> Option<&SymlinkEntry> {
        self.symlinks.iter().find(|e| e.link == link)
    }

    /// Update the last verified timestamp for a symlink
    pub fn update_verified(&mut self, link: &Path) {
        if let Some(entry) = self.symlinks.iter_mut().find(|e| e.link == link) {
            entry.last_verified = Some(chrono::Utc::now().to_rfc3339());
        }
    }

    /// Verify all symlinks and update their status
    /// Returns a list of (link_path, status) tuples where status indicates:
    /// - Ok(true): Symlink exists and points to correct target
    /// - Ok(false): Symlink exists but points to wrong target
    /// - Err: Symlink doesn't exist or there was an error checking it
    pub async fn verify_all(&mut self) -> Vec<(PathBuf, Result<bool>)> {
        let mut results = Vec::new();

        for entry in &mut self.symlinks {
            let status = Self::verify_symlink(&entry.link, &entry.target).await;

            if status.is_ok() {
                entry.last_verified = Some(chrono::Utc::now().to_rfc3339());
            }

            results.push((entry.link.clone(), status));
        }

        results
    }

    /// Verify a single symlink
    async fn verify_symlink(link: &Path, expected_target: &Path) -> Result<bool> {
        if !link.exists() && link.symlink_metadata().is_err() {
            return Err(anyhow::anyhow!("Symlink does not exist"));
        }

        let metadata = fs::symlink_metadata(link)
            .await
            .context("Failed to read symlink metadata")?;

        if !metadata.is_symlink() {
            return Err(anyhow::anyhow!("Path exists but is not a symlink"));
        }

        let actual_target = fs::read_link(link)
            .await
            .context("Failed to read symlink target")?;

        // Normalize paths for comparison
        let expected = normalize_path(expected_target)?;
        let actual = normalize_path(&actual_target)?;

        Ok(expected == actual)
    }
}

/// Create a symlink from `link` to `target`
/// Verifies the system state before creating and updates the state file
pub async fn create_symlink(link: &Path, target: &Path) -> Result<()> {
    log::debug!("Creating symlink: {:?} -> {:?}", link, target);

    // Verify target exists
    if !target.exists() {
        anyhow::bail!(
            "Target does not exist: {}. Cannot create symlink.",
            target.display()
        );
    }

    // Check if link already exists
    if link.symlink_metadata().is_ok() {
        let metadata = fs::symlink_metadata(link).await?;

        if metadata.is_symlink() {
            // It's a symlink - check if it points to the right place
            let current_target = fs::read_link(link).await?;
            let expected = normalize_path(target)?;
            let actual = normalize_path(&current_target)?;

            if expected == actual {
                log::debug!("Symlink already exists and points to correct target");

                // Update state
                let mut state = SymlinkState::load().await?;
                state.add_entry(link.to_path_buf(), target.to_path_buf());
                state.save().await?;

                return Ok(());
            } else {
                log::warn!(
                    "Symlink exists but points to wrong target. Current: {:?}, Expected: {:?}",
                    current_target,
                    target
                );
                anyhow::bail!(
                    "Symlink exists but points to {:?} instead of {:?}. \
                    Please remove it manually or use a different link path.",
                    current_target,
                    target
                );
            }
        } else {
            anyhow::bail!(
                "Path exists but is not a symlink: {}. \
                Please move or remove it before creating a symlink.",
                link.display()
            );
        }
    }

    // Create parent directory if needed
    if let Some(parent) = link.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create parent directory for symlink")?;
            log::debug!("Created parent directory: {:?}", parent);
        }
    }

    // Create the symlink
    #[cfg(unix)]
    fs::symlink(target, link)
        .await
        .context("Failed to create symlink")?;

    #[cfg(windows)]
    {
        if target.is_dir() {
            fs::symlink_dir(target, link)
                .await
                .context("Failed to create directory symlink")?;
        } else {
            fs::symlink_file(target, link)
                .await
                .context("Failed to create file symlink")?;
        }
    }

    log::debug!(
        "✓ Created symlink: {} -> {}",
        link.display(),
        target.display()
    );

    // Update state
    let mut state = SymlinkState::load().await?;
    state.add_entry(link.to_path_buf(), target.to_path_buf());
    state.save().await?;

    Ok(())
}

/// Remove a symlink and update the state file
/// Only removes if the path is actually a symlink
pub async fn remove_symlink(link: &Path) -> Result<()> {
    log::debug!("Removing symlink: {:?}", link);

    // Verify it's a symlink before removing
    if link.symlink_metadata().is_ok() {
        let metadata = fs::symlink_metadata(link).await?;

        if !metadata.is_symlink() {
            anyhow::bail!(
                "Path exists but is not a symlink: {}. Will not remove.",
                link.display()
            );
        }

        // Remove the symlink
        fs::remove_file(link)
            .await
            .context("Failed to remove symlink")?;

        log::debug!("✓ Removed symlink: {}", link.display());
    } else {
        log::warn!("Symlink does not exist: {:?}", link);
    }

    // Update state
    let mut state = SymlinkState::load().await?;
    state.remove_entry(link);
    state.save().await?;

    Ok(())
}

/// Verify a symlink and return its status
pub async fn verify_symlink(link: &Path, expected_target: &Path) -> Result<bool> {
    SymlinkState::verify_symlink(link, expected_target).await
}

/// List all managed symlinks with their status
pub async fn list_symlinks() -> Result<Vec<(SymlinkEntry, Result<bool>)>> {
    let state = SymlinkState::load().await?;
    let mut results = Vec::new();

    for entry in &state.symlinks {
        let status = SymlinkState::verify_symlink(&entry.link, &entry.target).await;
        results.push((entry.clone(), status));
    }

    Ok(results)
}

/// Clean up broken or invalid symlinks from the state
/// Returns the number of entries cleaned up
pub async fn cleanup_broken_symlinks() -> Result<usize> {
    let mut state = SymlinkState::load().await?;
    let original_count = state.symlinks.len();

    let mut to_remove = Vec::new();

    for entry in &state.symlinks {
        let status = SymlinkState::verify_symlink(&entry.link, &entry.target).await;

        // Remove entries where the symlink doesn't exist
        if status.is_err() {
            to_remove.push(entry.link.clone());
        }
    }

    for link in to_remove {
        state.remove_entry(&link);
    }

    let removed_count = original_count - state.symlinks.len();

    if removed_count > 0 {
        state.save().await?;
        log::info!("Cleaned up {} broken symlink entries", removed_count);
    }

    Ok(removed_count)
}

/// Normalize a path for comparison by resolving it to an absolute path
fn normalize_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        // For relative paths, try to make them absolute from current directory
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .context("Failed to normalize path")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symlink_state_default() {
        let state = SymlinkState::default();
        assert!(state.symlinks.is_empty());
    }

    #[test]
    fn test_add_entry() {
        let mut state = SymlinkState::default();
        let link = PathBuf::from("/home/user/.bashrc");
        let target = PathBuf::from("/home/user/dotfiles/bashrc");

        state.add_entry(link.clone(), target.clone());

        assert_eq!(state.symlinks.len(), 1);
        assert_eq!(state.symlinks[0].link, link);
        assert_eq!(state.symlinks[0].target, target);
    }

    #[test]
    fn test_remove_entry() {
        let mut state = SymlinkState::default();
        let link = PathBuf::from("/home/user/.bashrc");
        let target = PathBuf::from("/home/user/dotfiles/bashrc");

        state.add_entry(link.clone(), target);
        assert_eq!(state.symlinks.len(), 1);

        let removed = state.remove_entry(&link);
        assert!(removed);
        assert_eq!(state.symlinks.len(), 0);
    }

    #[test]
    fn test_find_entry() {
        let mut state = SymlinkState::default();
        let link = PathBuf::from("/home/user/.bashrc");
        let target = PathBuf::from("/home/user/dotfiles/bashrc");

        state.add_entry(link.clone(), target.clone());

        let found = state.find_entry(&link);
        assert!(found.is_some());
        assert_eq!(found.unwrap().target, target);

        let not_found = state.find_entry(Path::new("/nonexistent"));
        assert!(not_found.is_none());
    }
}

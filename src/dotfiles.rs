use anyhow::{Context, Result};
use dialoguer::{Select, theme::ColorfulTheme};
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::config::{Config, DotfileEntry, SourceType};
use crate::git;
use crate::symlinks;

/// Get the dotme configuration directory (~/.dotme)
pub fn get_dotme_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Failed to get home directory")?;
    Ok(home.join(".dotme"))
}

/// Get the default config file path (~/.dotme/config.yml)
pub fn get_config_path() -> Result<PathBuf> {
    Ok(get_dotme_dir()?.join("config.yml"))
}

/// Get the git repositories directory (~/.dotme/git)
pub fn get_git_dir() -> Result<PathBuf> {
    Ok(get_dotme_dir()?.join("git"))
}

/// Initialize the dotme configuration
pub async fn init() -> Result<()> {
    let dotme_dir = get_dotme_dir()?;
    let config_path = get_config_path()?;
    let git_dir = get_git_dir()?;

    if config_path.exists() {
        log::info!("DotMe is already initialized at {}", dotme_dir.display());
        return Ok(());
    }

    log::info!("Initializing DotMe at {}", dotme_dir.display());

    // Create .dotme directory
    fs::create_dir_all(&dotme_dir)
        .await
        .context("Failed to create .dotme directory")?;

    // Create git directory for storing cloned repositories
    fs::create_dir_all(&git_dir)
        .await
        .context("Failed to create git directory")?;

    // Create initial config file
    let config = Config::default();
    config.save(&config_path)?;

    log::info!("DotMe initialized successfully!");
    log::info!("Config file created at {}", config_path.display());
    log::info!("Git repositories will be stored in {}", git_dir.display());

    Ok(())
}

/// Detect the type of source based on its format/path
fn detect_source_type(source: &str) -> Result<SourceType> {
    // Check for git repository patterns
    if source.starts_with("https://")
        && (source.ends_with(".git")
            || source.contains("github.com")
            || source.contains("gitlab.com"))
    {
        return Ok(SourceType::Git);
    }

    if source.starts_with("http://")
        && (source.ends_with(".git")
            || source.contains("github.com")
            || source.contains("gitlab.com"))
    {
        return Ok(SourceType::Git);
    }

    if source.starts_with("git@") || source.starts_with("ssh://git@") {
        return Ok(SourceType::Git);
    }

    // For local paths, check if they exist
    let path = Path::new(source);

    // Handle relative and absolute paths
    let expanded_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    if expanded_path.is_dir() {
        return Ok(SourceType::Directory);
    }

    if expanded_path.is_file() {
        return Ok(SourceType::File);
    }

    anyhow::bail!(
        "Could not determine source type for '{}'. Path does not exist or is not a valid git repository URL.",
        source
    )
}

/// Add a new dotfile entry
pub async fn add(
    source: &str,
    target: Option<PathBuf>,
    path: Option<PathBuf>,
    folders: Option<Vec<String>>,
    dry_run: bool,
) -> Result<()> {
    let config_path = get_config_path()?;

    if !config_path.exists() {
        anyhow::bail!("DotMe is not initialized. Run 'dotme init' first.");
    }

    let mut config = Config::load(Some(config_path.clone()))?;

    // Detect source type
    let source_type = detect_source_type(source)?;

    log::info!("Detected source type: {}", source_type);

    // Determine the base path for symlinks (where they will be created)
    let base_path = if let Some(ref p) = path {
        p.clone()
    } else {
        // Default to current working directory
        std::env::current_dir().context("Failed to get current working directory")?
    };

    log::debug!("Symlinks will be created in: {}", base_path.display());

    // Determine target location
    let target = if let Some(t) = target {
        t
    } else {
        // For git repos, store in ~/.dotme/git directory
        if matches!(source_type, SourceType::Git) {
            let git_dir = get_git_dir()?;
            // Extract repo name from git URL
            let repo_name = source
                .rsplit('/')
                .next()
                .unwrap_or("repo")
                .trim_end_matches(".git");
            git_dir.join(repo_name)
        } else {
            // For local files/directories, use base_path
            let source_path = Path::new(source);
            let filename = source_path
                .file_name()
                .context("Failed to get filename from source")?;
            base_path.join(filename)
        }
    };

    // Check for duplicates
    if config.dotfiles.iter().any(|e| e.source == source) {
        anyhow::bail!("Source '{}' is already being managed", source);
    }

    // For git repositories, clone them immediately
    let selected_folders = if matches!(source_type, SourceType::Git) {
        // Check if git is available
        git::check_git_available().await?;

        // Clone the repository
        git::clone(source, &target).await?;

        // If path is set, skip folder selection and use repo root (None means entire repo)
        // This overrides any --folders flag to ensure root-level symlinking
        if path.is_some() {
            if folders.is_some() {
                log::warn!("--path flag overrides --folders; symlinking from repository root");
            } else {
                log::info!("Path specified, symlinking from repository root");
            }
            None
        } else if folders.is_none() {
            // If folders weren't specified via CLI and no path, prompt the user
            prompt_folder_selection(&target).await?
        } else {
            folders
        }
    } else {
        folders
    };

    // Create final entry with selected folders
    let entry = DotfileEntry {
        source: source.to_string(),
        target: target.clone(),
        r#type: source_type,
        path: Some(base_path.clone()),
        folders: selected_folders,
    };

    config.dotfiles.push(entry.clone());
    config.save(&config_path)?;

    log::info!("Added '{}' to dotfiles management", source);

    // Create symlinks for the newly added entry
    if dry_run {
        println!("\n[DRY RUN] Symlinks that would be created:");
        create_symlinks_for_entry(&entry, &base_path, dry_run).await?;
    } else {
        log::info!("Creating symlinks...");
        create_symlinks_for_entry(&entry, &base_path, dry_run).await?;
    }

    Ok(())
}

/// Show status of managed dotfiles
pub async fn status() -> Result<()> {
    let config_path = get_config_path()?;

    if !config_path.exists() {
        println!("DotMe is not initialized. Run 'dotme init' to set up dotfiles management.");
        return Ok(());
    }

    let config = Config::load(Some(config_path))?;

    if config.dotfiles.is_empty() {
        println!("No dotfiles are currently being managed.");
        println!("Use 'dotme add <source>' to add dotfiles.");
        return Ok(());
    }

    println!("Managed Dotfiles:");
    if let Some(updated) = &config.updated {
        println!("Last updated: {}", format_timestamp(updated));
    }
    println!("─────────────────────────────────────────");

    for entry in &config.dotfiles {
        let status = if entry.target.exists() {
            "✓ exists"
        } else {
            "✗ missing"
        };

        println!("  {} [{}]", status, entry.r#type);
        println!("    Source: {}", entry.source);

        // For git repos, show they're stored in ~/.dotme/git
        if matches!(entry.r#type, SourceType::Git) {
            println!("    Local:  {}", entry.target.display());
            if let Some(folders) = &entry.folders {
                println!("    Folders: {}", folders.join(", "));
            }
        } else {
            println!("    Target: {}", entry.target.display());
        }
        println!();
    }

    Ok(())
}

/// Update all managed dotfiles
pub async fn update(dry_run: bool) -> Result<()> {
    let config_path = get_config_path()?;

    if !config_path.exists() {
        anyhow::bail!("DotMe is not initialized. Run 'dotme init' first.");
    }

    let mut config = Config::load(Some(config_path.clone()))?;

    if config.dotfiles.is_empty() {
        log::info!("No dotfiles to update.");
        return Ok(());
    }

    if dry_run {
        println!("\n[DRY RUN] Update operation - showing what would be done:\n");
    }

    log::info!("Updating {} dotfile(s)...", config.dotfiles.len());

    for entry in &config.dotfiles {
        log::info!("Processing: {} [{}]", entry.source, entry.r#type);

        // Determine the base path for symlinks
        let base_path = if let Some(ref p) = entry.path {
            p.clone()
        } else {
            // Default to home directory for backward compatibility
            dirs::home_dir().context("Failed to get home directory")?
        };

        // Remove old symlinks before creating new ones
        remove_symlinks_for_entry(entry, Some(&base_path), dry_run).await?;

        match entry.r#type {
            SourceType::File => {
                // For files, we create symlinks instead of copying
                create_symlinks_for_entry(entry, &base_path, dry_run).await?;
            }
            SourceType::Directory => {
                // For directories, we create symlinks instead of copying
                create_symlinks_for_entry(entry, &base_path, dry_run).await?;
            }
            SourceType::Git => {
                // If repository doesn't exist, clone it
                if !entry.target.exists() {
                    if dry_run {
                        println!("[DRY RUN] Would clone repository: {}", entry.source);
                    } else {
                        log::info!("Repository not found, cloning...");
                        git::clone(&entry.source, &entry.target).await?;
                    }
                } else {
                    // Otherwise, pull latest changes
                    if dry_run {
                        println!("[DRY RUN] Would pull latest changes from: {}", entry.source);
                    } else {
                        git::pull(&entry.target).await?;
                    }
                }

                // Create symlinks for git repository folders
                create_symlinks_for_entry(entry, &base_path, dry_run).await?;
            }
        }
    }

    // Update the timestamp (only if not dry run)
    if !dry_run {
        config.update_timestamp();
        config.save(&config_path)?;
    } else {
        println!("\n[DRY RUN] Would update timestamp in config");
    }

    log::info!("Update complete!");

    Ok(())
}

async fn copy_file(source: &str, target: &Path) -> Result<()> {
    let source_path = Path::new(source);

    if !source_path.exists() {
        anyhow::bail!("Source file '{}' does not exist", source);
    }

    // Create parent directory if needed
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).await?;
    }

    fs::copy(source_path, target).await.context(format!(
        "Failed to copy {} to {}",
        source,
        target.display()
    ))?;

    log::info!("  ✓ Copied to {}", target.display());

    Ok(())
}

async fn copy_directory(source: &str, target: &Path) -> Result<()> {
    let source_path = Path::new(source);

    if !source_path.exists() {
        anyhow::bail!("Source directory '{}' does not exist", source);
    }

    fs::create_dir_all(target).await?;

    let mut entries = fs::read_dir(source_path).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let file_name = path.file_name().unwrap();
        let target_path = target.join(file_name);

        if path.is_dir() {
            Box::pin(copy_directory(path.to_str().unwrap(), &target_path)).await?;
        } else {
            fs::copy(&path, &target_path).await?;
        }
    }

    log::info!("  ✓ Copied directory to {}", target.display());

    Ok(())
}

/// Remove a dotfile entry from management
pub async fn remove(source: Option<String>) -> Result<()> {
    let config_path = get_config_path()?;

    if !config_path.exists() {
        anyhow::bail!("DotMe is not initialized. Run 'dotme init' first.");
    }

    let mut config = Config::load(Some(config_path.clone()))?;

    if config.dotfiles.is_empty() {
        log::info!("No dotfiles are currently being managed.");
        return Ok(());
    }

    // Determine which entry to remove
    let entry_to_remove = if let Some(src) = source {
        // Find the entry by source
        config
            .dotfiles
            .iter()
            .find(|e| e.source == src)
            .ok_or_else(|| anyhow::anyhow!("Source '{}' is not being managed", src))?
            .clone()
    } else {
        // Interactive selection
        let items: Vec<String> = config
            .dotfiles
            .iter()
            .map(|e| format!("[{}] {}", e.r#type, e.source))
            .collect();

        if items.is_empty() {
            log::info!("No dotfiles to remove.");
            return Ok(());
        }

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select dotfile to remove")
            .items(&items)
            .default(0)
            .interact()?;

        config.dotfiles[selection].clone()
    };

    log::info!("Removing '{}' from management", entry_to_remove.source);

    // Remove associated symlinks
    log::info!("Removing associated symlinks...");
    let removed_count = remove_symlinks_for_entry(&entry_to_remove, None, false).await?;
    if removed_count > 0 {
        log::info!("✓ Removed {} symlink(s)", removed_count);
    } else {
        log::info!("No symlinks to remove");
    }

    // If it's a git repository, remove the cloned directory
    if matches!(entry_to_remove.r#type, SourceType::Git) {
        if entry_to_remove.target.exists() {
            log::info!(
                "Deleting git repository at: {}",
                entry_to_remove.target.display()
            );
            fs::remove_dir_all(&entry_to_remove.target)
                .await
                .context("Failed to remove git repository directory")?;
            log::info!("✓ Git repository deleted");
        }
    }

    // Remove from config
    config
        .dotfiles
        .retain(|e| e.source != entry_to_remove.source);
    config.save(&config_path)?;

    log::info!(
        "✓ Removed '{}' from dotfiles management",
        entry_to_remove.source
    );

    Ok(())
}

/// Prompt user to select folders from a git repository
async fn prompt_folder_selection(repo_path: &Path) -> Result<Option<Vec<String>>> {
    use dialoguer::{MultiSelect, theme::ColorfulTheme};

    // Get all top-level directories in the repository
    let mut folders = Vec::new();
    let mut entries = fs::read_dir(repo_path).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        // Skip hidden files/directories and .git directory
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if !name.starts_with('.') && path.is_dir() {
                folders.push(name.to_string());
            }
        }
    }

    if folders.is_empty() {
        log::info!("No folders found in repository");
        return Ok(None);
    }

    // Sort folders alphabetically
    folders.sort();

    // Add "All folders" as the first option
    let mut display_items = vec!["All folders".to_string()];
    display_items.extend(folders.clone());

    println!("\nSelect folders to sync to your home directory:");
    println!("(Use Space to select/deselect, Enter to confirm)");

    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select folders")
        .items(&display_items)
        .interact()?;

    if selections.is_empty() {
        log::info!("No folders selected, repository will be managed without folder filtering");
        return Ok(None);
    }

    // Check if "All folders" (index 0) was selected
    if selections.contains(&0) {
        log::info!("All folders selected");
        return Ok(None);
    }

    // Map selections back to folder names (subtract 1 because of "All folders" option)
    let selected_folders: Vec<String> =
        selections.iter().map(|&i| folders[i - 1].clone()).collect();

    if selected_folders.is_empty() {
        Ok(None)
    } else {
        log::info!("Selected folders: {}", selected_folders.join(", "));
        Ok(Some(selected_folders))
    }
}

/// Sync specific folders from a git repository to the home directory
async fn sync_git_folders(repo_path: &Path, folders: &[String]) -> Result<()> {
    let home = dirs::home_dir().context("Failed to get home directory")?;

    for folder in folders {
        let source_folder = repo_path.join(folder);

        if !source_folder.exists() {
            log::warn!("Folder '{}' does not exist in repository, skipping", folder);
            continue;
        }

        if !source_folder.is_dir() {
            log::warn!("'{}' is not a directory, skipping", folder);
            continue;
        }

        let target_folder = home.join(folder);

        log::info!("Syncing folder '{}' to {}", folder, target_folder.display());

        // Copy the folder recursively
        copy_directory(source_folder.to_str().unwrap(), &target_folder).await?;
    }

    Ok(())
}

/// Format a timestamp for display
fn format_timestamp(timestamp: &str) -> String {
    use chrono::{DateTime, Local};

    if let Ok(dt) = timestamp.parse::<DateTime<chrono::Utc>>() {
        let local: DateTime<Local> = dt.into();
        local.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        timestamp.to_string()
    }
}

/// Remove all symlinks associated with a dotfile entry
async fn remove_symlinks_for_entry(
    entry: &DotfileEntry,
    _base_path: Option<&Path>,
    dry_run: bool,
) -> Result<usize> {
    use crate::symlinks::SymlinkState;

    // Load symlink state
    let state = SymlinkState::load().await?;

    if state.symlinks.is_empty() {
        return Ok(0);
    }

    let mut removed_count = 0;
    let mut symlinks_to_remove = Vec::new();

    // Determine the base path to match against
    let target_path = match entry.r#type {
        SourceType::File => {
            // For files, the target is the actual file path
            Path::new(&entry.source).to_path_buf()
        }
        SourceType::Directory => {
            // For directories, match any symlink that points into this directory
            Path::new(&entry.source).to_path_buf()
        }
        SourceType::Git => {
            // For git repos, match any symlink that points into the cloned repo
            entry.target.clone()
        }
    };

    log::debug!("Looking for symlinks pointing to: {:?}", target_path);

    // Find all symlinks that point to paths under the target path
    for symlink_entry in &state.symlinks {
        // Check if the symlink target starts with the target path
        if symlink_entry.target.starts_with(&target_path) {
            log::debug!(
                "Found symlink to remove: {} -> {}",
                symlink_entry.link.display(),
                symlink_entry.target.display()
            );
            symlinks_to_remove.push(symlink_entry.link.clone());
        }
    }

    // Remove the symlinks
    for link in &symlinks_to_remove {
        if dry_run {
            println!("[DRY RUN] Would remove old symlink: {}", link.display());
            removed_count += 1;
        } else {
            match symlinks::remove_symlink(link).await {
                Ok(_) => {
                    removed_count += 1;
                    log::info!("  ✓ Removed symlink: {}", link.display());
                }
                Err(e) => {
                    log::warn!("  ✗ Failed to remove symlink {}: {}", link.display(), e);
                }
            }
        }
    }

    Ok(removed_count)
}

/// Create symlinks for a dotfile entry following the symlink creation rules
async fn create_symlinks_for_entry(
    entry: &DotfileEntry,
    base_path: &Path,
    dry_run: bool,
) -> Result<()> {
    match entry.r#type {
        SourceType::File => {
            // For files: create symlink if target doesn't exist
            let source_path = Path::new(&entry.source);
            let filename = source_path.file_name().context("Failed to get filename")?;
            let target_path = base_path.join(filename);

            create_symlink_if_needed(&target_path, source_path, dry_run).await?;
        }
        SourceType::Directory => {
            // For directories: process contents and create symlinks in base_path
            let source_path = Path::new(&entry.source);

            // Process each item in the source directory
            let mut entries_list = fs::read_dir(source_path).await?;

            while let Some(dir_entry) = entries_list.next_entry().await? {
                let item_path = dir_entry.path();
                let item_name = item_path.file_name().context("Failed to get item name")?;

                // Skip .git directory
                if item_name == ".git" {
                    continue;
                }

                let target_path = base_path.join(item_name);

                if item_path.is_dir() {
                    process_directory_for_symlinks(&item_path, &target_path, dry_run).await?;
                } else {
                    create_symlink_if_needed(&target_path, &item_path, dry_run).await?;
                }
            }
        }
        SourceType::Git => {
            // For git repos: handle selected folders or entire repo

            if let Some(folders) = &entry.folders {
                // Process only selected folders
                for folder in folders {
                    let source_folder = entry.target.join(folder);

                    if !source_folder.exists() {
                        log::warn!("Folder '{}' does not exist in repository, skipping", folder);
                        continue;
                    }

                    log::info!("Processing folder: {}", folder);

                    // Process the CONTENTS of the folder, not the folder itself
                    // This creates symlinks from items inside the folder to the base_path
                    let mut entries_list = fs::read_dir(&source_folder).await?;

                    while let Some(dir_entry) = entries_list.next_entry().await? {
                        let item_path = dir_entry.path();
                        let item_name = item_path.file_name().context("Failed to get item name")?;

                        // Skip .git directory
                        if item_name == ".git" {
                            continue;
                        }

                        let target_path = base_path.join(item_name);

                        if item_path.is_dir() {
                            process_directory_for_symlinks(&item_path, &target_path, dry_run)
                                .await?;
                        } else {
                            create_symlink_if_needed(&target_path, &item_path, dry_run).await?;
                        }
                    }
                }
            } else {
                // Process entire repository - also process contents, not the repo folder itself
                let mut entries_list = fs::read_dir(&entry.target).await?;

                while let Some(dir_entry) = entries_list.next_entry().await? {
                    let item_path = dir_entry.path();
                    let item_name = item_path.file_name().context("Failed to get item name")?;

                    // Skip .git directory
                    if item_name == ".git" {
                        continue;
                    }

                    let target_path = base_path.join(item_name);

                    if item_path.is_dir() {
                        process_directory_for_symlinks(&item_path, &target_path, dry_run).await?;
                    } else {
                        create_symlink_if_needed(&target_path, &item_path, dry_run).await?;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Process a directory recursively to create symlinks following the rules
async fn process_directory_for_symlinks(
    source_dir: &Path,
    target_dir: &Path,
    dry_run: bool,
) -> Result<()> {
    log::debug!("Processing directory: {:?} -> {:?}", source_dir, target_dir);

    if !source_dir.exists() {
        log::warn!("Source directory does not exist: {:?}", source_dir);
        return Ok(());
    }

    // Check if target already exists (including broken symlinks)
    if target_dir.symlink_metadata().is_ok() {
        // Target exists (file, directory, or symlink) - check what it is
        if target_dir.is_dir() {
            // Rule 2: Target is a directory, descend into it
            log::debug!("Target directory exists, processing contents recursively");

            let mut entries = fs::read_dir(source_dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let source_path = entry.path();
                let item_name = source_path.file_name().context("Failed to get item name")?;

                // Skip .git directory
                if item_name == ".git" {
                    log::debug!("Skipping .git directory");
                    continue;
                }

                let target_path = target_dir.join(item_name);

                if source_path.is_dir() {
                    // Recursively process subdirectory (use Box::pin for async recursion)
                    Box::pin(process_directory_for_symlinks(
                        &source_path,
                        &target_path,
                        dry_run,
                    ))
                    .await?;
                } else {
                    // Process file
                    create_symlink_if_needed(&target_path, &source_path, dry_run).await?;
                }
            }
        } else {
            // Rule 3: Target exists as a file/symlink - skip
            log::debug!("Target exists as file/symlink, skipping: {:?}", target_dir);
            if dry_run {
                println!("[DRY RUN] Would skip (exists): {}", target_dir.display());
            }
        }
    } else {
        // Rule 1: Target doesn't exist, create symlink to entire directory
        log::debug!("Target directory doesn't exist, creating symlink to entire directory");
        if dry_run {
            println!(
                "[DRY RUN] Would create symlink: {} -> {}",
                target_dir.display(),
                source_dir.display()
            );
        } else {
            symlinks::create_symlink(target_dir, source_dir).await?;
        }
    }

    Ok(())
}

/// Create a symlink if the target doesn't exist (Rule 1) or skip if it exists (Rule 3)
async fn create_symlink_if_needed(link: &Path, target: &Path, dry_run: bool) -> Result<()> {
    // Check if target (link location) exists
    if link.exists() || link.symlink_metadata().is_ok() {
        // Rule 3: Target exists - skip (never overwrite)
        log::debug!("Path already exists, skipping: {:?}", link);
        if dry_run {
            println!("[DRY RUN] Would skip (exists): {}", link.display());
        }
        return Ok(());
    }

    // Rule 1: Target doesn't exist - create symlink
    log::debug!("Creating symlink: {:?} -> {:?}", link, target);

    // Verify source exists before creating symlink
    if !target.exists() {
        log::warn!("Source does not exist, cannot create symlink: {:?}", target);
        if dry_run {
            println!(
                "[DRY RUN] Would skip (source missing): {} -> {}",
                link.display(),
                target.display()
            );
        }
        return Ok(());
    }

    if dry_run {
        println!(
            "[DRY RUN] Would create symlink: {} -> {}",
            link.display(),
            target.display()
        );
    } else {
        // Create the symlink (this also tracks it in symlinks.yml)
        symlinks::create_symlink(link, target).await?;
    }

    Ok(())
}

/// List all currently applied symlinks
pub async fn list() -> Result<()> {
    log::info!("Loading symlink state...");

    let symlinks = symlinks::list_symlinks().await?;

    if symlinks.is_empty() {
        println!("No symlinks are currently managed by DotMe.");
        println!("Use 'dotme add <source>' to add dotfiles and create symlinks.");
        return Ok(());
    }

    println!("Managed Symlinks:");
    println!("─────────────────────────────────────────");

    for (entry, status) in symlinks {
        let status_str = match status {
            Ok(true) => "✓ valid",
            Ok(false) => "⚠ points to wrong target",
            Err(_) => "✗ broken or missing",
        };

        println!("  {} {}", status_str, entry.link.display());
        println!("    → {}", entry.target.display());
        println!("    Created: {}", format_timestamp(&entry.created_at));
        if let Some(verified) = &entry.last_verified {
            println!("    Verified: {}", format_timestamp(verified));
        }
        println!();
    }

    Ok(())
}

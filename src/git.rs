use anyhow::{Context, Result};
use std::path::Path;
use tokio::process::Command;

/// Clone a git repository to the specified path
pub async fn clone(url: &str, target: &Path) -> Result<()> {
    log::info!("Cloning git repository: {}", url);
    log::debug!("Target path: {}", target.display());

    // Check if target already exists
    if target.exists() {
        log::warn!("Target directory already exists: {}", target.display());
        return Ok(());
    }

    // Create parent directory if needed
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("Failed to create parent directory")?;
    }

    // Clone the repository
    let output = Command::new("git")
        .arg("clone")
        .arg(url)
        .arg(target)
        .output()
        .await
        .context("Failed to execute git clone command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Git clone failed: {}", stderr);
    }

    log::info!("✓ Repository cloned successfully");

    // Check for .gitmodules file and initialize submodules if present
    let gitmodules_path = target.join(".gitmodules");
    if gitmodules_path.exists() {
        log::info!("Found .gitmodules file, initializing submodules...");
        init_submodules(target).await?;
    }

    Ok(())
}

/// Initialize and update git submodules
async fn init_submodules(repo_path: &Path) -> Result<()> {
    log::debug!("Initializing submodules in: {}", repo_path.display());

    // Initialize submodules
    let init_output = Command::new("git")
        .arg("submodule")
        .arg("init")
        .current_dir(repo_path)
        .output()
        .await
        .context("Failed to execute git submodule init")?;

    if !init_output.status.success() {
        let stderr = String::from_utf8_lossy(&init_output.stderr);
        anyhow::bail!("Git submodule init failed: {}", stderr);
    }

    // Update submodules
    let update_output = Command::new("git")
        .arg("submodule")
        .arg("update")
        .arg("--recursive")
        .current_dir(repo_path)
        .output()
        .await
        .context("Failed to execute git submodule update")?;

    if !update_output.status.success() {
        let stderr = String::from_utf8_lossy(&update_output.stderr);
        anyhow::bail!("Git submodule update failed: {}", stderr);
    }

    log::info!("✓ Submodules initialized and updated");

    Ok(())
}

/// Pull latest changes from a git repository
pub async fn pull(repo_path: &Path) -> Result<()> {
    log::info!("Pulling latest changes: {}", repo_path.display());

    if !repo_path.exists() {
        anyhow::bail!("Repository does not exist: {}", repo_path.display());
    }

    // Pull changes
    let output = Command::new("git")
        .arg("pull")
        .current_dir(repo_path)
        .output()
        .await
        .context("Failed to execute git pull")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Git pull failed: {}", stderr);
    }

    log::info!("✓ Repository updated successfully");

    // Update submodules if .gitmodules exists
    let gitmodules_path = repo_path.join(".gitmodules");
    if gitmodules_path.exists() {
        log::info!("Updating submodules...");
        update_submodules(repo_path).await?;
    }

    Ok(())
}

/// Update git submodules
async fn update_submodules(repo_path: &Path) -> Result<()> {
    let output = Command::new("git")
        .arg("submodule")
        .arg("update")
        .arg("--recursive")
        .arg("--remote")
        .current_dir(repo_path)
        .output()
        .await
        .context("Failed to execute git submodule update")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Git submodule update failed: {}", stderr);
    }

    log::info!("✓ Submodules updated");

    Ok(())
}

/// Check if git is available on the system
pub async fn check_git_available() -> Result<()> {
    let output = Command::new("git")
        .arg("--version")
        .output()
        .await
        .context("Failed to check git version. Is git installed?")?;

    if !output.status.success() {
        anyhow::bail!("Git command failed. Please ensure git is installed.");
    }

    let version = String::from_utf8_lossy(&output.stdout);
    log::debug!("Git version: {}", version.trim());

    Ok(())
}

/// Get the current status of a git repository
#[allow(dead_code)]
pub async fn status(repo_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .arg("status")
        .arg("--short")
        .current_dir(repo_path)
        .output()
        .await
        .context("Failed to execute git status")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Git status failed: {}", stderr);
    }

    let status = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(status)
}

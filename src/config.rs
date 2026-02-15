use anyhow::{Context, Result};
use figment::{
    Figment,
    providers::{Env, Format, Json, Toml, Yaml},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const PROJECT_NAME: &str = env!("CARGO_PKG_NAME");

/// Type of dotfile source
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SourceType {
    File,
    Directory,
    Git,
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceType::File => write!(f, "file"),
            SourceType::Directory => write!(f, "directory"),
            SourceType::Git => write!(f, "git"),
        }
    }
}

/// Dotfile entry configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DotfileEntry {
    /// Source path or git repository URL
    pub source: String,
    /// Target location in the filesystem
    pub target: PathBuf,
    /// Type of source
    #[serde(rename = "type")]
    pub r#type: SourceType,
    /// Path where symlinks should be created (defaults to home directory if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// Optional folders to select (only for git repositories)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folders: Option<Vec<String>>,
}

/// Paths configuration for dotme directories and files
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathsConfig {
    /// Path to the dotme directory (default: ~/.dotme)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dotme_dir: Option<PathBuf>,
    /// Path to the git repositories directory (default: ~/.dotme/git)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_dir: Option<PathBuf>,
    /// Path to the symlinks state file (default: ~/.dotme/symlinks.yml)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symlinks_file: Option<PathBuf>,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            dotme_dir: None,
            git_dir: None,
            symlinks_file: None,
        }
    }
}

impl PathsConfig {
    /// Get the dotme directory path, using configured value or default
    pub fn get_dotme_dir(&self) -> Result<PathBuf> {
        if let Some(ref path) = self.dotme_dir {
            Ok(path.clone())
        } else {
            let home = dirs::home_dir().context("Failed to get home directory")?;
            Ok(home.join(".dotme"))
        }
    }

    /// Get the git directory path, using configured value or default
    pub fn get_git_dir(&self) -> Result<PathBuf> {
        if let Some(ref path) = self.git_dir {
            Ok(path.clone())
        } else {
            Ok(self.get_dotme_dir()?.join("git"))
        }
    }

    /// Get the symlinks file path, using configured value or default
    pub fn get_symlinks_file(&self) -> Result<PathBuf> {
        if let Some(ref path) = self.symlinks_file {
            Ok(path.clone())
        } else {
            Ok(self.get_dotme_dir()?.join("symlinks.yml"))
        }
    }
}

/// Configuration settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    /// Last time dotme update was run (ISO 8601 format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    /// Paths configuration
    #[serde(default)]
    pub paths: PathsConfig,
    /// List of managed dotfiles
    #[serde(default)]
    pub dotfiles: Vec<DotfileEntry>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            updated: None,
            paths: PathsConfig::default(),
            dotfiles: Vec::new(),
        }
    }
}

impl Config {
    /// Load configuration from both environment variables and a configuration file
    pub fn load(path: Option<PathBuf>) -> Result<Self> {
        let path = match path {
            Some(p) => p,
            None => {
                let home = dirs::home_dir().context("Failed to get home directory")?;
                home.join(".dotme").join("config.yml")
            }
        };

        let path = path.as_path();
        log::debug!("Loading configuration from {}", path.display());

        let project_name = PROJECT_NAME.to_uppercase();
        log::debug!("Loading environment prefix: {}", project_name);
        let mut fig = Figment::new().merge(Env::prefixed(project_name.as_str()));

        if path.exists() {
            if path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
            {
                log::debug!("Loading configuration from YAML file");
                fig = fig.merge(Yaml::file(path));
            } else if path.extension().is_some_and(|ext| ext == "toml") {
                log::debug!("Loading configuration from TOML file");
                fig = fig.merge(Toml::file(path));
            } else if path.extension().is_some_and(|ext| ext == "json") {
                log::debug!("Loading configuration from JSON file");
                fig = fig.merge(Json::file(path));
            } else {
                log::warn!("Unsupported configuration file format");
                return Err(anyhow::anyhow!("Unsupported configuration file format"));
            }
        } else {
            log::warn!("Configuration file not found");
        }

        Ok(fig.extract()?)
    }

    /// Update the last updated timestamp to current time
    pub fn update_timestamp(&mut self) {
        self.updated = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Update configuration with command line arguments
    #[allow(dead_code, unused)]
    pub fn arguments(&mut self, arguments: &crate::cli::Arguments) {
        todo!("Lets write some code...");
    }

    /// Save configuration to a file
    #[allow(dead_code)]
    pub fn save(&self, path: impl Into<PathBuf>) -> Result<()> {
        let path = path.into();
        log::debug!("Saving configuration to {}", path.display());

        let data = if path
            .extension()
            .is_some_and(|ext| ext == "yaml" || ext == "yml")
        {
            serde_yaml::to_string(self)?
        } else if path.extension().is_some_and(|ext| ext == "toml") {
            toml::to_string(self)?
        } else if path.extension().is_some_and(|ext| ext == "json") {
            serde_json::to_string(self)?
        } else {
            log::warn!("Unsupported configuration file format");
            return Err(anyhow::anyhow!("Unsupported configuration file format"));
        };

        std::fs::write(path, data)?;

        Ok(())
    }
}

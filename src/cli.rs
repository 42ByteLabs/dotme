use clap::{Parser, Subcommand};
use console::style;
use std::path::PathBuf;

pub const VERSION_NUMBER: &str = env!("CARGO_PKG_VERSION");
pub const AUTHOR: &str = env!("CARGO_PKG_AUTHORS");

pub const BANNER: &str = r#"
________          __     _____
\______ \   _____/  |_  /     \   ____
 |    |  \ /  _ \   __\/  \ /  \_/ __ \
 |    `   (  <_> )  | /    Y    \  ___/
/_______  /\____/|__| \____|__  /\___  >
        \/                    \/     \/"#;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Arguments {
    /// Enable Debugging
    #[clap(long, env, default_value_t = false)]
    pub debug: bool,

    /// Disable Banner
    #[clap(long, default_value_t = false)]
    pub disable_banner: bool,

    /// Configuration file path (defaults to ~/.dotme/config.yml)
    #[clap(short, long, env)]
    pub config: Option<PathBuf>,

    /// Subcommands
    #[clap(subcommand)]
    pub commands: Option<ArgumentCommands>,
}

#[derive(Subcommand, Debug)]
pub enum ArgumentCommands {
    /// Initialize dotfiles management
    Init,
    /// Add a file, directory, or git repository to dotfiles management
    Add {
        /// Path to file, directory, or git repository URL
        source: String,
        /// Optional target location (defaults to home directory)
        #[clap(short, long)]
        target: Option<PathBuf>,
        /// Path where symlinks should be created (defaults to current working directory)
        #[clap(short, long)]
        path: Option<PathBuf>,
        /// Select specific folders from git repository (comma-separated, e.g., "dev,geek")
        #[clap(short, long, value_delimiter = ',')]
        folders: Option<Vec<String>>,
        /// Dry run mode - show what would be done without creating symlinks
        #[clap(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Update/sync all managed dotfiles
    Update {
        /// Dry run mode - show what would be done without creating symlinks
        #[clap(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Show status of managed dotfiles
    Status,
    /// Remove a dotfile entry from management
    Remove {
        /// Source path or git repository URL to remove (optional - will prompt if not provided)
        source: Option<String>,
    },
    /// List all currently applied symlinks
    List,
}

pub fn init() -> Arguments {
    let arguments = Arguments::parse();

    let log_level = match &arguments.debug {
        false => log::LevelFilter::Info,
        true => log::LevelFilter::Debug,
    };

    env_logger::builder()
        .parse_default_env()
        .filter_level(log_level)
        .init();

    if !arguments.disable_banner {
        println!(
            "{}    {} - v{}",
            style(BANNER).green(),
            style(AUTHOR).red(),
            style(VERSION_NUMBER).blue()
        );
    }

    arguments
}

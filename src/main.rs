#![doc = include_str!("../README.md")]
#![allow(dead_code)]
#![deny(missing_docs)]
#![forbid(unsafe_code)]

use anyhow::Result;
use log::{debug, error};

mod cli;
mod config;
mod dotfiles;
mod git;
mod symlinks;

use crate::cli::*;

#[tokio::main]
async fn main() -> Result<()> {
    let arguments = init();
    debug!("Finished initialising, starting main workflow...");

    // Handle subcommands
    match &arguments.commands {
        None => {
            // No subcommand provided - show status
            if let Err(e) = dotfiles::status().await {
                error!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(ArgumentCommands::Init) => {
            if let Err(e) = dotfiles::init().await {
                error!("Failed to initialize: {}", e);
                std::process::exit(1);
            }
        }
        Some(ArgumentCommands::Add {
            source,
            target,
            path,
            folders,
            dry_run,
        }) => {
            if let Err(e) = dotfiles::add(source, target.clone(), path.clone(), folders.clone(), *dry_run).await {
                error!("Failed to add dotfile: {}", e);
                std::process::exit(1);
            }
        }
        Some(ArgumentCommands::Update { dry_run }) => {
            if let Err(e) = dotfiles::update(*dry_run).await {
                error!("Failed to update dotfiles: {}", e);
                std::process::exit(1);
            }
        }
        Some(ArgumentCommands::Status) => {
            if let Err(e) = dotfiles::status().await {
                error!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(ArgumentCommands::Remove { source }) => {
            if let Err(e) = dotfiles::remove(source.clone()).await {
                error!("Failed to remove dotfile: {}", e);
                std::process::exit(1);
            }
        }
        Some(ArgumentCommands::List) => {
            if let Err(e) = dotfiles::list().await {
                error!("Failed to list symlinks: {}", e);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

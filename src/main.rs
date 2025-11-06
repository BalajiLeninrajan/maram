mod cli;
mod config;
mod git;
mod metadata;
mod worktree_set;
mod zellij;

use anyhow::Result;
use clap::Parser;
use cli::{
    Cli, Commands, handle_checkout, handle_create, handle_delete, handle_diff, handle_pick,
    handle_reset, handle_status,
};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Create { branch_name } => handle_create(branch_name),
        Commands::Checkout { branch_name } => handle_checkout(branch_name),
        Commands::Delete { branch_name } => handle_delete(branch_name),
        Commands::Status => handle_status(),
        Commands::Pick { variant_name } => handle_pick(variant_name),
        Commands::Reset => handle_reset(),
        Commands::Diff { variant1, variant2 } => handle_diff(variant1, variant2),
    }
}

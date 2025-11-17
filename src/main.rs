mod cli;
mod commands;
mod config;
mod git;
mod metadata;
mod worktree_set;
mod zellij;

use crate::{cli::handle_list, commands::Commands};
use anyhow::Result;
use clap::Parser;
use cli::{
    Cli, handle_add, handle_checkout, handle_create, handle_delete, handle_diff, handle_pick,
    handle_remove, handle_reset, handle_status, handle_sync,
};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Create {
            branch_name,
            no_session,
            variants,
        } => handle_create(branch_name, no_session, variants),
        Commands::Checkout {
            branch_name,
            no_session,
        } => handle_checkout(branch_name, no_session),
        Commands::Delete { branch_name } => handle_delete(branch_name),
        Commands::Status => handle_status(),
        Commands::List => handle_list(),
        Commands::Pick { variant_name } => handle_pick(variant_name),
        Commands::Reset => handle_reset(),
        Commands::Diff { variant1, variant2 } => handle_diff(variant1, variant2),
        Commands::Add { variant_name } => handle_add(variant_name),
        Commands::Remove { variant_name } => handle_remove(variant_name),
        Commands::Sync => handle_sync(),
    }
}

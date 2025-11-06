mod cli;
mod config;
mod git;
mod metadata;
mod worktree_set;
mod zellij;

use anyhow::Result;
use clap::Parser;
use cli::{
    handle_checkout, handle_create, handle_delete, handle_diff, handle_pick, handle_reset,
    handle_status, Cli, Commands,
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
        Commands::Pick { variant_name } => handle_pick(variant_name),
        Commands::Reset => handle_reset(),
        Commands::Diff { variant1, variant2 } => handle_diff(variant1, variant2),
    }
}

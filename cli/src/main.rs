mod archive_cmd;
mod args;
#[cfg(test)]
mod args_tests;
#[cfg(test)]
mod boundary_tests;
mod config_args;
mod config_cmd;
#[cfg(test)]
mod credential_args_tests;
mod dev_cmd;
mod import_cmd;
mod import_input;
#[cfg(test)]
mod import_tests;
mod insomnia_cmd;
mod migration_cmd;
mod phy_cmd;
mod rel_cmd;
mod repair_cmd;
mod util;
mod vectors_cmd;

use anyhow::Result;
use args::{Cli, Command};
use clap::Parser;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Rel { command } => rel_cmd::run(command),
        Command::Phy { command } => phy_cmd::run(command),
        Command::Migrate { source, output } => migration_cmd::run(&source, &output),
        Command::MigrateConversationTitles { source, rel } => {
            migration_cmd::run_conversation_titles(&source, &rel)
        }
        Command::Import { command } => import_cmd::run(command),
        Command::Archive { command } => archive_cmd::run(command),
        Command::Config { command } => config_cmd::run(&cli.config, command),
        Command::Vectors { command } => vectors_cmd::run(&cli.config, command),
        Command::Insomnia { command } => insomnia_cmd::run(&cli.config, command),
        Command::Dev { command } => dev_cmd::run(command),
    }
}

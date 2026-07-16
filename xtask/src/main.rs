use anyhow::Result;
use chrono::Local;
use clap::{Parser, Subcommand};
use xtask::{changelog, release};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the CHANGELOG.md section for a released version
    ExtractChangelog { version: String },
    /// Bump Cargo.toml, date the CHANGELOG Unreleased section, run the gate, and commit
    PrepareRelease { version: String },
    /// Push the release branch and open a draft PR against mozilla/stmo-cli main
    CutRelease { version: String },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let repo_root = std::env::current_dir()?;
    match cli.command {
        Command::ExtractChangelog { version } => {
            let changelog = std::fs::read_to_string("CHANGELOG.md")?;
            let section = changelog::extract_changelog_section(&changelog, &version)?;
            print!("{section}");
        }
        Command::PrepareRelease { version } => {
            let today = Local::now().format("%Y-%m-%d").to_string();
            release::prepare_release(&repo_root, &version, &today)?;
        }
        Command::CutRelease { version } => {
            release::cut_release(&repo_root, &version)?;
        }
    }
    Ok(())
}

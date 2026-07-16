#![allow(clippy::missing_errors_doc)]

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

// Returns a git Command with inherited git env vars cleared, so commands run against
// a different repo (e.g. a worktree) are not affected by a parent worktree's GIT_DIR
// or GIT_INDEX_FILE. Mirrors src/commands/init.rs::clean_git_cmd.
#[must_use]
pub fn clean_git_cmd() -> Command {
    let mut cmd = Command::new("git");
    cmd.env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_INDEX_FILE");
    cmd
}

pub fn run(repo_root: &Path, args: &[&str]) -> Result<()> {
    let status = clean_git_cmd()
        .args(args)
        .current_dir(repo_root)
        .status()
        .with_context(|| format!("failed to run git {args:?}"))?;
    if !status.success() {
        anyhow::bail!("git {args:?} failed");
    }
    Ok(())
}

pub fn output(repo_root: &Path, args: &[&str]) -> Result<String> {
    let out = clean_git_cmd()
        .args(args)
        .current_dir(repo_root)
        .output()
        .with_context(|| format!("failed to run git {args:?}"))?;
    if !out.status.success() {
        anyhow::bail!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
